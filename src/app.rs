use crate::jackit;
use crate::portbuf;
use crate::comm::{self, Update};

use egui::plot::{Line, Plot, PlotPoints};

pub struct TemplateApp {
    jackit: jackit::JackIt,
    port_bufs: Vec<portbuf::PortBuf>,
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(jackit: jackit::JackIt, port_bufs: Vec<portbuf::PortBuf>) -> Self {
        TemplateApp {
	    jackit,
            port_bufs,
        }
    }
}

impl  eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        //let Self { rx, .. } = self;

        egui::CentralPanel::default().show(ctx, |ui| {
            // let line = Line::new(PlotPoints::from_ys_f32(self.aview.get_n(1024)));
            // Plot::new("my_plot")
            //     .view_aspect(2.0)
            //     .show(ui, |plot_ui| plot_ui.line(line));
        });
    }

    fn on_close_event(&mut self) -> bool {
	self.port_bufs.iter_mut().for_each(|pb| pb.quit());
	self.jackit.stop();
	std::thread::sleep(comm::PORT_BUF_WAIT_DUR);
	true
    }
}
