mod app;
mod comm;
mod jackit;
mod portbuf;

use anyhow::Result;
use app::TemplateApp;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "scviz",
        native_options,
        Box::new(|cc| {
            let ctx = cc.egui_ctx.clone();

            let port_names: Vec<String> = vec!["in_1", "in_2", "in_3"]
                .iter()
                .map(|s| s.to_string())
                .collect();

	    let mut jackit = jackit::JackIt::new("scviz", port_names.clone());

            // size of the buffer jack is configured to hand out each process cycle
            let jack_buf_size = jackit.buffer_size();

            // sample rate jack is configured to run at
            let jack_sample_rate = jackit.sample_rate();

            println!(
                "jack_sample_rate = {}  jack_buffer_size = {}",
                jack_sample_rate, jack_buf_size
            );

	    let bus = comm::Bus::new(ctx.clone());
            let ringbuf_consumers = jackit
                .start(jackit::JackItConfig {
		    bus: bus.clone(),
                    ringbuf_cycle_size: comm::RINGBUF_CYCLE_SIZE,
                })
                .expect("JackIt to activate");

	    // consume ring buffers in activated port_bufs
	    let port_bufs: Vec<portbuf::PortBuf> = ringbuf_consumers.into_iter().map(|rb| {
		let mut pb = portbuf::PortBuf::new(comm::PORT_BUF_SIZE);
		pb.activate(portbuf::PortBufProcessConfig {
                    rb,
                    agg_bin_size: jack_buf_size as usize, // aggregate every jack process buffer
                    wait_dur: comm::PORT_BUF_WAIT_DUR,
                    ctx: ctx.clone(),
                }).expect("PortBuf Activate to Succeed");
		pb
	    }).collect();


            Box::new(TemplateApp::new(bus, jackit, port_bufs))
        }),
    )?;
    Ok(())
}
