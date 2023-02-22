mod app;
mod comm;
mod jackit;
mod portbuf;

use app::TemplateApp;
use crossbeam_channel;

fn main() {
    tracing_subscriber::fmt::init();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "scviz",
        native_options,
        Box::new(|cc| {
            let (tx, rx) = crossbeam_channel::unbounded();
            let ctx = cc.egui_ctx.clone();
            let jackit = jackit::JackIt::new(ctx, tx);
            Box::new(TemplateApp::new(jackit, rx))
        }),
    );
}
