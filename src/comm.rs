use std::sync::Arc;

pub type RingProducer = ringbuf::producer::Producer<f32, Arc<ringbuf::HeapRb<f32>>>;
pub type RingConsumer = ringbuf::consumer::Consumer<f32, Arc<ringbuf::HeapRb<f32>>>;

/// How many Jack process cycles can fit into the ringbuf
pub const RINGBUF_CYCLE_SIZE: usize = 10;

/// How many point pairs can the underlying port buffers hold before overwriting
pub const PORT_BUF_SIZE: usize = 65_536;

/// How many samples to aggregate over for the time series buffer. For efficiency
/// chosen to match the typical jack_buffer_size.
pub const AGG_SAMPLE_SIZE: usize = 1024;

/// How long should the port buff wait between ring buffer reads
pub const PORT_BUF_WAIT_DUR: std::time::Duration = std::time::Duration::from_millis(1);

/// The number of FFT Samples to aggregate over. Divisible by 1024 and a power of 2
/// for greatest efficiency
pub const FFT_BUF_SIZE: usize = 16_384;

/// The size of the main channel bus
pub const CHANNEL_BUS_SIZE: usize = 10;

/// Number of Process Cycles to include in diagnostic aggregation
pub const TIMING_DIAGNOSTIC_CYCLES: u32 = 10;

// pub const AUDIO_BUFF_SIZE: usize = 8192;
// pub const FFT_MAX_SIZE: usize = 8192;
// pub const FFT_MAX_BUFF_SIZE: usize = 4097;
// pub const MAX_DATA_LENGTH: usize = 10000;
// pub const APP_WIDTH: f32 = 1200.0;
// pub const APP_HEIGHT: f32 = 800.0;

#[derive(Debug)]
pub enum Update {
    Jack(Jack),
    PortBuf(PortBuf),
}

#[derive(Debug)]
pub enum Jack {
    Connected {
        connected: bool,
        port_names: Vec<String>,
    },
    TimingDiagnostics(TimingDiagnostics),
}

#[derive(Debug)]
pub enum PortBuf {
    TimingDiagnostics {
        port_idx: usize,
        timing: TimingDiagnostics,
    },
}

#[derive(Clone)]
pub struct Bus {
    ctx: egui::Context,
    tx: crossbeam_channel::Sender<Update>,
    rx: crossbeam_channel::Receiver<Update>,
}

impl Bus {
    pub fn new(ctx: egui::Context) -> Bus {
        let (tx, rx) = crossbeam_channel::bounded(CHANNEL_BUS_SIZE);
        Bus { ctx, tx, rx }
    }

    pub fn send(&self, updt: Update) {
        self.tx.send(updt).expect("Bus.send to succeed");
        self.ctx.request_repaint();
    }

    pub fn updates(&self, debug: bool) -> Vec<Update> {
        let updts: Vec<Update> = self.rx.try_iter().collect();
        if debug {
            for updt in updts.iter() {
                println!("{:?}", updt);
            }
        }
        updts
    }

    pub fn request_repaint(&self) {
        self.ctx.request_repaint();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimingDiagnostics {
    diagnostic_proc_cycles: u32,
    diagnostic_proc_cycle: u32,
    now: std::time::Instant,
    pub avg_diag_cycle_time: std::time::Duration,
    pub max_diag_cycle_time: std::time::Duration,
    pub min_diag_cycle_time: std::time::Duration,
}

impl TimingDiagnostics {
    pub fn new(diagnostic_proc_cycles: u32) -> Self {
        TimingDiagnostics {
            diagnostic_proc_cycles,
            diagnostic_proc_cycle: 0,
            now: std::time::Instant::now(),
            avg_diag_cycle_time: std::time::Duration::ZERO,
            max_diag_cycle_time: std::time::Duration::ZERO,
            min_diag_cycle_time: std::time::Duration::MAX,
        }
    }

    pub fn record(&mut self) {
        if self.diagnostic_proc_cycle == self.diagnostic_proc_cycles {
            self.diagnostic_proc_cycle = 0;
            self.avg_diag_cycle_time = std::time::Duration::ZERO;
            self.max_diag_cycle_time = std::time::Duration::ZERO;
            self.min_diag_cycle_time = std::time::Duration::MAX;
        }
        self.now = std::time::Instant::now();
    }

    pub fn done(&mut self) -> bool {
        let elapsed = self.now.elapsed();
        self.avg_diag_cycle_time += elapsed / self.diagnostic_proc_cycles;
        self.max_diag_cycle_time = self.max_diag_cycle_time.max(elapsed);
        self.min_diag_cycle_time = self.min_diag_cycle_time.min(elapsed);
        self.diagnostic_proc_cycle += 1;
        self.diagnostic_proc_cycle == self.diagnostic_proc_cycles
    }
}
