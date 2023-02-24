use std::sync::Arc;

pub type RingProducer = ringbuf::producer::Producer<[f32; 2], Arc<ringbuf::HeapRb<[f32; 2]>>>;
pub type RingConsumer = ringbuf::consumer::Consumer<[f32; 2], Arc<ringbuf::HeapRb<[f32; 2]>>>;

/// How many Jack process cycles can fit into the ringbuf
pub const RINGBUF_CYCLE_SIZE: usize = 10;

/// How many point pairs can the underlying port buffers hold before overwriting
pub const PORT_BUF_SIZE: usize = 65536;

/// How long should the port buff wait between ring buffer reads
pub const PORT_BUF_WAIT_DUR: std::time::Duration = std::time::Duration::from_millis(10);

/// The size of the main channel bus
pub const CHANNEL_BUS_SIZE: usize = 10;

// pub const AUDIO_BUFF_SIZE: usize = 8192;
// pub const FFT_MAX_SIZE: usize = 8192;
// pub const FFT_MAX_BUFF_SIZE: usize = 4097;
// pub const MAX_DATA_LENGTH: usize = 10000;
// pub const APP_WIDTH: f32 = 1200.0;
// pub const APP_HEIGHT: f32 = 800.0;

#[derive(Debug)]
pub enum Update {
    Jack(Jack),
}

#[derive(Debug)]
pub enum Jack {
    Connected {
        connected: bool,
        port_names: Vec<String>,
    },
    ProcessTime(std::time::Duration),
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
}
