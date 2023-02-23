use std::sync::Arc;

pub type RingProducer = ringbuf::producer::Producer<[f32; 2], Arc<ringbuf::HeapRb<[f32; 2]>>>;
pub type RingConsumer = ringbuf::consumer::Consumer<[f32; 2], Arc<ringbuf::HeapRb<[f32; 2]>>>;

/// how many Jack process cycles can fit into the ringbuf
pub const RINGBUF_CYCLE_SIZE: usize = 10;

/// how many point pairs can the underlying port buffers hold before overwriting
pub const PORT_BUF_SIZE: usize = 65536;

/// how long should the port buff wait between ring buffer reads
pub const PORT_BUF_WAIT_DUR: std::time::Duration = std::time::Duration::from_millis(10);

// pub const AUDIO_BUFF_SIZE: usize = 8192;
// pub const FFT_MAX_SIZE: usize = 8192;
// pub const FFT_MAX_BUFF_SIZE: usize = 4097;
// pub const MAX_DATA_LENGTH: usize = 10000;
// pub const APP_WIDTH: f32 = 1200.0;
// pub const APP_HEIGHT: f32 = 800.0;


pub enum Update {
    Jack(Jack),
}

pub enum Jack {
    Connected{connected: bool, port_names: Vec<String>}
}
