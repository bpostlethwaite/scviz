use crate::app;
use crate::comm::{self, Update};
use anyhow::Result;
use std::sync::{Arc, Mutex};

const AGG_SAMPLES: usize = 1024;
const AGG_LEN: usize = 65536;
const RAW_LEN: usize = 65536;
const FFT_SIG_BUF_SIZE: usize = 8192;
const FFT_SPEC_BUF_SIZE: usize = 4097;

#[derive(Debug)]
struct ArrayView {
    len: usize,
    arr: Vec<[f64; 2]>,
}

impl ArrayView {
    fn new(capacity: usize) -> ArrayView {
        ArrayView {
            len: 0,
            arr: vec![[0.0, 0.0]; capacity],
        }
    }

    fn push(&mut self, x: f64, y: f64) {
        if self.len == self.arr.capacity() {
            self.len = 0;
        }
        self.arr[self.len] = [x, y];
        self.len += 1;
    }

    fn get_last(&mut self, n: usize) -> &[[f64; 2]] {
        if n > self.len {
            &self.arr[0..self.len]
        } else {
            let start = self.len - n;
            &self.arr[start..self.len]
        }
    }
}

struct TriBuf {
    agg: ArrayView,
    raw: ArrayView,
}

pub struct PortBufProcessConfig {
    pub agg_bin_size: usize,
    pub rb: comm::RingConsumer,
    pub wait_dur: std::time::Duration,
    pub bus: comm::Bus,
}

pub struct PortBuf {
    port_idx: usize,
    buf: Arc<Mutex<TriBuf>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
    quit_tx: Option<crossbeam_channel::Sender<()>>,
}

impl PortBuf {
    pub fn new(port_idx: usize, buf_capacity: usize) -> PortBuf {
        PortBuf {
	    port_idx,
            buf: Arc::new(Mutex::new(TriBuf {
                agg: ArrayView::new(buf_capacity),
                raw: ArrayView::new(buf_capacity),
            })),
            join_handle: None,
            quit_tx: None,
        }
    }

    pub fn activate(&mut self, config: PortBufProcessConfig) -> Result<()> {
        let arcbuf = self.buf.clone();
        let (quit_tx, quit_rx) = crossbeam_channel::bounded(1);
        self.quit_tx = Some(quit_tx);

        let PortBufProcessConfig {
            mut rb,
            agg_bin_size,
            wait_dur,
            bus,
        } = config;

        let join_handle = std::thread::spawn(move || {
            let mut fft_idx = 0;
            let mut fft_tmp_buf: [f32; FFT_SIG_BUF_SIZE * 2] = [0.0; FFT_SIG_BUF_SIZE * 2];
            let mut timing_diagnostics = comm::TimingDiagnostics::new(comm::TIMING_DIAGNOSTIC_CYCLES);
            loop {
                timing_diagnostics.record();
                match quit_rx.try_recv() {
                    Ok(_) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => break,
                    Err(crossbeam_channel::TryRecvError::Empty) => (),
                }

                if rb.len() < agg_bin_size {
                    match quit_rx.recv_timeout(wait_dur) {
                        Ok(_) => break,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
			    continue
			},
                    }
                }

                let mut n_samples = rb.len();
                // Prevent fft_buf overflow
                if rb.len() >= FFT_SIG_BUF_SIZE * 2 {
                    n_samples = FFT_SIG_BUF_SIZE * 2;
                }

                // round take down to divisible amount of agg_bin_size - usize division is a floor op
                n_samples = (n_samples / agg_bin_size) * agg_bin_size;
                let mut sum = 0.0;
                {
                    let mut buf = match arcbuf.lock() {
                        Ok(buf) => buf,
                        Err(_) => break,
                    };
                    // Take only an amount that is both divisible by agg_sample sizes, and less than
                    // FFT_SIG_BUFF_SIZE*2 leave rest for next process loop.
                    for (idx, [x, t]) in rb.pop_iter().take(n_samples).enumerate() {
                        sum += x;
                        // aggregate every agg_bin_size
                        // TODO need to compute agg t by looking at current frame time and adding
                        // idx sample times - 0.5 agg_bin_size samples to it.
                        if ((idx + 1) % agg_bin_size) == 0 {
                            buf.agg.push(sum as f64 / agg_bin_size as f64, t as f64);
                            sum = 0.0;
                        }
                        buf.raw.push(x as f64, t as f64);
                        fft_tmp_buf[fft_idx] = x;
                        fft_idx += 1;
                    }
                }

                // Handle FFT - for now just reset the index
                if fft_tmp_buf.len() > FFT_SIG_BUF_SIZE {
                    fft_idx = 0;
                }
                if timing_diagnostics.done() {
                    bus.send(Update::PortBuf(comm::PortBuf::TimingDiagnostics(
                        timing_diagnostics,
                    )))
                } else {
                    bus.request_repaint();
		}
            }
        });

        self.join_handle = Some(join_handle);

        Ok(())
    }

    pub fn quit(&mut self) {
        match self.quit_tx.take() {
            Some(quit_tx) => quit_tx.send(()).expect("PortBuf quit tx to send"),
            None => (),
        }
        match self.join_handle.take() {
            Some(join_handle) => join_handle
                .join()
                .expect("PortBuf join - thread has panicked"),
            None => (),
        }
        println!("PortBuf Stopped");
    }

    pub fn update(&self, updts: &Vec<Update>, state: &mut app::State) {
        for updt in updts {
            match updt {
                Update::PortBuf(comm::PortBuf::TimingDiagnostics(d)) => {
                    state.ports[self.port_idx].timing  = *d
                }
                _ => (),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_view() {
        let mut av = ArrayView::new(2);
        av.push(2.0, 2.0);
        av.push(3.0, 3.0);

        let points = av.get_last(2);
        assert!(points.len() == 2);

        av.push(4.0, 4.0);

        let points = av.get_last(2);
        assert!(points.len() == 1);
        let p1 = points[0];
        assert!(p1[0] == 4.0 && p1[1] == 4.0);
    }

    #[test]
    fn port_buf() {
        // set portbuf capacity at 5 and agg_bin_size at 2
        // then write 5 values at once. portbuf should pull 2
        // values each loop and leave the 5th value.
        let mut pbuf = PortBuf::new(0, 4);

        // create a ringbuffer of capacity 5
        let rb = ringbuf::HeapRb::new(5);
        let (mut prod, cons) = rb.split();
        let wait_dur = std::time::Duration::from_millis(10);
        let process_config = PortBufProcessConfig {
            rb: cons,
            agg_bin_size: 2,
            wait_dur,
            bus: comm::Bus::new(egui::Context::default()),
        };

        pbuf.activate(process_config).expect("pbuf to not throw");

        let mut test_data = vec![[0.0, 0.0]; 5];
        for i in 1..test_data.len() {
            test_data[i][0] = (i * 2) as f32;
            test_data[i][1] = i as f32;
        }
        prod.push_slice(&test_data);

        // wait some time after the first process will have woken up
        std::thread::sleep(wait_dur + std::time::Duration::from_millis(5));

        pbuf.quit();

        let buf = pbuf.buf.lock().expect("tribuf to unlock");
        assert!(buf.agg.len == 2);
        assert!(buf.agg.arr[0][0] == 1.0);
        assert!(buf.agg.arr[1][0] == 5.0);
        assert!(buf.raw.len == 4);
        assert!(prod.len() == 1);
    }
}
