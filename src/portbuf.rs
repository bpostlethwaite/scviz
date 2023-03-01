use crate::comm::{self, TimingDiagnostics, Update, FFT_BUF_SIZE};
use anyhow::Result;
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct ArrayView<const N: usize> {
    idx: usize,
    cycled: bool,
    rise_cycle: bool,
    rising_idx: usize,
    x_thresh: f32,
    arr: [f32; N],
}

impl<const N: usize> ArrayView<N> {
    fn new() -> ArrayView<N> {
        ArrayView {
            idx: 0,
            cycled: false,
            rise_cycle: false,
            rising_idx: 0,
            x_thresh: 0.0,
            arr: [0.0; N],
        }
    }

    fn push(&mut self, x: f32) {
        self.arr[self.idx] = x;
        if self.idx + 1 == N {
            self.idx = 0;
            self.cycled = true;
        } else {
            self.idx = self.idx + 1;
        }
    }

    // TODO: Optimize push_slice
    fn push_slice(&mut self, xs: &[f32]) {
	for x in xs.iter() {
            self.arr[self.idx] = *x;
            if self.idx + 1 == N {
		self.idx = 0;
		self.cycled = true;
            } else {
		self.idx = self.idx + 1;
            }
	}
    }

    fn push_slice_with_rise(&mut self, xs: &[f32]) {
	for &x in xs.iter() {
            if self.cycled || self.idx > 1 {
		if self.rise_cycle && x < self.x_thresh {
                    self.rise_cycle = false;
		}

		if !self.rise_cycle && x > self.x_thresh {
                    self.rise_cycle = true;
                    self.rising_idx = self.idx;
		}
            }

            self.arr[self.idx] = x;
            if self.idx + 1 == N {
		self.idx = 0;
		self.cycled = true;
            } else {
		self.idx = self.idx + 1;
            }
	}

    }

    /// As Push but also sets last idx when x value crosses
    /// x_thresh and is rising
    fn push_riser(&mut self, x: f32) {
        if self.cycled || self.idx > 1 {
            if self.rise_cycle && x < self.x_thresh {
                self.rise_cycle = false;
            }

            if !self.rise_cycle && x > self.x_thresh {
                self.rise_cycle = true;
                self.rising_idx = self.idx;
            }
        }
        self.push(x);
    }

    fn clear(&mut self) {
        self.idx = 0;
        self.cycled = false;
    }

    fn last(&self) -> f32 {
        if self.idx == 0 {
            self.arr[N]
        } else {
            self.arr[self.idx - 1]
        }
    }

    fn last_n(&self, n: usize) -> Vec<f32> {
        debug_assert!(n <= N);
        let mut vec = Vec::with_capacity(n);
        if self.cycled {
            if self.idx >= n {
                vec.extend(&self.arr[(self.idx - n)..self.idx]);
            } else {
                vec.extend(&self.arr[(N - n + self.idx)..N]);
                vec.extend(&self.arr[0..self.idx]);
            }
        } else {
            if self.idx <= n {
                vec.extend(&self.arr[0..self.idx]);
            } else {
                vec.extend(&self.arr[self.idx - n..self.idx]);
            }
        }
        vec
    }

    fn last_nt(&mut self, n: usize, t_start: f64, dt: f64) -> Vec<[f64; 2]> {
        debug_assert!(n <= N);
        let mut vec = Vec::with_capacity(n);
        let mut t = t_start;
        if self.cycled {
            if self.idx >= n {
                for i in (self.idx - n)..self.idx {
                    vec.push([t, self.arr[i] as f64]);
                    t += dt;
                }
            } else {
                for i in ((N - n + self.idx)..N).chain(0..self.idx) {
                    vec.push([t, self.arr[i] as f64]);
                    t += dt;
                }
            }
        } else {
            if self.idx <= n {
                for i in 0..self.idx {
                    vec.push([t, self.arr[i] as f64]);
                    t += dt;
                }
            } else {
                for i in self.idx - n..self.idx {
                    vec.push([t, self.arr[i] as f64]);
                    t += dt;
                }
            }
        }
        vec
    }

    fn last_nt_rising(&mut self, n: usize, t_start: f64, dt: f64) -> Vec<[f64; 2]> {
        debug_assert!(n <= N);
        let idx = self.rising_idx;
        let mut vec = Vec::with_capacity(n);
        let mut t = t_start;
        if self.cycled {
            if idx >= n {
                for i in (idx - n)..idx {
                    vec.push([t, self.arr[i] as f64]);
                    t += dt;
                }
            } else {
                for i in ((N - n + idx)..N).chain(0..idx) {
                    vec.push([t, self.arr[i] as f64]);
                    t += dt;
                }
            }
        } else {
            if idx <= n {
                for i in 0..idx {
                    vec.push([t, self.arr[i] as f64]);
                    t += dt;
                }
            } else {
                for i in idx - n..idx {
                    vec.push([t, self.arr[i] as f64]);
                    t += dt;
                }
            }
        }
        vec
    }

    fn size(&self) -> usize {
        N
    }

    fn idx(&self) -> usize {
        self.idx
    }
}

struct TriBuf<const N: usize> {
    agg: ArrayView<N>,
    raw: ArrayView<N>,
}

pub struct PortBufProcessConfig {
    pub agg_bin_size: usize,
    pub rb: comm::RingConsumer,
    pub bus: comm::Bus,
}

pub struct PortBuf<const N: usize> {
    pub name: String,
    pub timing: TimingDiagnostics,
    pub enabled: bool,
    pub sample_rate: usize,
    buf: Arc<Mutex<TriBuf<N>>>,
    port_idx: usize,
    join_handle: Option<std::thread::JoinHandle<()>>,
    quit_tx: Option<crossbeam_channel::Sender<()>>,
}

impl<const N: usize> PortBuf<N> {
    pub fn new(port_idx: usize, name: String, enabled: bool, sample_rate: usize) -> PortBuf<N> {
        PortBuf {
            name,
            enabled,
            port_idx,
            sample_rate,
            timing: TimingDiagnostics::new(0),
            buf: Arc::new(Mutex::new(TriBuf {
                agg: ArrayView::new(),
                raw: ArrayView::new(),
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
            bus,
        } = config;

        // Logic assumes this is true as we pull agg_bin_size
        // quantities off the ring_buffer at a time.
        debug_assert!(FFT_BUF_SIZE % agg_bin_size == 0);

        let port_idx = self.port_idx;
        let join_handle = std::thread::spawn(move || {
            //let mut planner = FftPlanner::new();
            //let fft = planner.plan_fft_forward(FFT_BUF_SIZE);
            let mut data_buf: [f32; FFT_BUF_SIZE] = [0.0; FFT_BUF_SIZE];
            let mut fft_buf: [Complex<f32>; FFT_BUF_SIZE] = [Complex {
                re: 0.0f32,
                im: 0.0f32,
            }; FFT_BUF_SIZE];
            let mut fft_scratch_buf: [Complex<f32>; FFT_BUF_SIZE] = [Complex {
                re: 0.0f32,
                im: 0.0f32,
            }; FFT_BUF_SIZE];
            let mut timing_diagnostics = TimingDiagnostics::new(comm::TIMING_DIAGNOSTIC_CYCLES);
            let mut fft_idx = 0;

            loop {
                if cfg!(debug_assertions) {
                    timing_diagnostics.record()
                };
                match quit_rx.try_recv() {
                    Ok(_) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => break,
                    Err(crossbeam_channel::TryRecvError::Empty) => (),
                }

                // we need at least agg_bin_size
                if rb.len() < agg_bin_size {
                    match quit_rx.recv_timeout(comm::PORT_BUF_WAIT_DUR) {
                        Ok(_) => break,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                    }
                }

                // Take only enough to fill whatever remains in the FFT_BUFFER
                // FFT_BUFF_SIZE is divisible by agg_bin_size.
                let rb_len = rb.len().min(FFT_BUF_SIZE - fft_idx);
                let agg_chunks = rb_len / agg_bin_size;
                let n_samples = agg_chunks * agg_bin_size;

                // fill up our internal data buffer
                let data_slice = &mut data_buf[0..n_samples];
		{
                    rb.pop_slice(data_slice);
		}
                // calculate the aggs
                let aggs: Vec<f32> = data_slice
                    .chunks_exact(agg_bin_size)
                    .map(|chunk| chunk.iter().sum::<f32>() / agg_bin_size as f32)
                    .collect();

                // load and possibly calculate the ffts
		for x in data_slice.iter() {
		    fft_buf[fft_idx] = Complex::new(*x, 0.0);
		    fft_idx += 1;
		}
		if fft_idx == FFT_BUF_SIZE {
		    fft_idx = 0;
		}

		// Unlock Buf
                {
                    let mut buf = match arcbuf.lock() {
                        Ok(buf) => buf,
                        Err(_) => break,
                    };
                    buf.raw.push_slice_with_rise(data_slice);
		    buf.agg.push_slice(&aggs);
		    //buf.fft.push_slice(fft_buf);
                }
		// Relinquish Lock

                if cfg!(debug_assertions) {
                    if timing_diagnostics.done() {
                        bus.send(Update::PortBuf(comm::PortBuf::TimingDiagnostics {
                            timing: timing_diagnostics,
                            port_idx,
                        }))
                    } else {
                        bus.request_repaint();
                    }
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

    pub fn curr_idx(&self) -> (usize, usize) {
        let buf = self
            .buf
            .lock()
            .expect("PortBuf.capcity lock to not be poisoned");
        (buf.agg.idx(), buf.raw.idx())
    }

    pub fn update(&mut self, updts: &Vec<Update>) {
        for updt in updts {
            match updt {
                Update::PortBuf(comm::PortBuf::TimingDiagnostics { port_idx, timing }) => {
                    if port_idx == &self.port_idx {
                        self.timing = *timing;
                    }
                }
                Update::Jack(comm::Jack::Connected {
                    connected,
                    port_names,
                }) => {
                    for port_name in port_names.into_iter() {
                        if port_name == &self.name {
                            self.enabled = *connected;
                        }
                    }
                }
                _ => (),
            }
        }
    }

    pub fn time_window(&self, tw: f64, t_start: f64) -> Vec<[f64; 2]> {
        let sample_time = 1.0 / self.sample_rate as f64;
        let period = tw;
        let samples_per_period = (period / sample_time).ceil() as usize;
        let mut buf = self
            .buf
            .lock()
            .expect("PortBuf raw_n lock to not be poisoned");
        buf.raw
            .last_nt_rising(samples_per_period, t_start, sample_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_view() {
        let mut av: ArrayView<10> = ArrayView::new();
        av.push(2.0);
        av.push(3.0);

        let points = av.last_n(2);
        assert!(points.len() == 2);

        av.push(4.0);

        let points = av.last_n(2);
        assert!(points[0] == 3.0);
        assert!(points[1] == 4.0);
    }

    #[test]
    fn array_view_push_rise() {
        let mut av: ArrayView<20> = ArrayView::new();
        for i in -5..5 {
            av.push_riser(i as f32);
        }
        for i in -5..5 {
            av.push_riser(i as f32);
        }

        println!("{:?}", av);
        // let points = av.last_n(2);
        // assert!(points.len() == 2);

        // let points = av.last_n(2);
        // assert!(points[0] == 3.0);
        // assert!(points[1] == 4.0);
    }

    #[test]
    fn port_buf() {
        // set portbuf capacity at 5 and agg_bin_size at 2
        // then write 5 values at once. portbuf should pull 2
        // values each loop and leave the 5th value.
        let mut pbuf: PortBuf<10> = PortBuf::new(0, "name".to_owned(), true, 48_000);

        // create a ringbuffer of capacity 5
        let rb = ringbuf::HeapRb::new(5);
        let (mut prod, cons) = rb.split();
        let process_config = PortBufProcessConfig {
            rb: cons,
            agg_bin_size: 2,
            bus: comm::Bus::new(egui::Context::default()),
        };

        pbuf.activate(process_config).expect("pbuf to not throw");

        let mut test_data = vec![0.0; 5];
        for i in 1..test_data.len() {
            test_data[i] = (i * 2) as f32;
        }
        // [0, 2, 4, 6, 8, 10]
        prod.push_slice(&test_data);

        // wait some time after the first process will have woken up
        std::thread::sleep(comm::PORT_BUF_WAIT_DUR + std::time::Duration::from_millis(5));

        pbuf.quit();
        let buf = pbuf.buf.lock().expect("tribuf to unlock");
        assert!(buf.agg.len() == 2);
        assert!(buf.agg.arr[0] == 1.0);
        assert!(buf.agg.arr[1] == 5.0);
        assert!(buf.raw.len() == 4);
        assert!(prod.len() == 1);
    }
}
