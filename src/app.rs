use crate::jackit;
use crate::portbuf;
use crate::comm::{self, Update};

use egui::plot::{Line, Plot, PlotPoints};


pub struct State {
    pub ports_enabled: Vec<bool>,
    pub jack_process_time: std::time::Duration,
}

impl State {
    fn new(n_ports: usize) -> State {
	State {
	    jack_process_time: std::time::Duration::ZERO,
	    ports_enabled: vec![false; n_ports],
	}
    }
}
pub struct TemplateApp {

    // sub-systems
    jackit: jackit::JackIt,
    port_bufs: Vec<portbuf::PortBuf>,
    bus: comm::Bus,

    // app state
    state: State,
}

impl TemplateApp {
    pub fn new(bus: comm::Bus, jackit: jackit::JackIt, port_bufs: Vec<portbuf::PortBuf>) -> Self {
	let state = State::new(port_bufs.len());
        TemplateApp {
	    jackit,
            port_bufs,
	    bus,
	    state,
        }
    }
}

impl  eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self { state, .. } = self;

	let updates = self.bus.updates(false);
	self.jackit.update(updates, state);

	egui::SidePanel::right("diagnostics").show(ctx, |ui| {
	    ui.heading("Status");
	    for (port_name, enabled) in std::iter::zip(self.jackit.port_names(), &state.ports_enabled) {
		ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
		    ui.label(port_name);
		    if *enabled {
			ui.label("☑");
		    } else {
			ui.label("☐");
		    }
		});
	    }
	    ui.separator();
	    ui.heading("Perf");
	    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
		ui.label("Jack Process Time: ");
		ui.label(format!("{:?}", state.jack_process_time));
	    });
	});

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
