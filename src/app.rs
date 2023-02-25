use crate::comm;
use crate::jackit;
use crate::portbuf;

use egui::plot::{Line, Plot, PlotPoints};

pub struct TemplateApp {
    // sub-systems
    jackit: jackit::JackIt,
    portbufs: Vec<portbuf::PortBuf>,
    bus: comm::Bus,
    plots: Vec<Box<dyn XPlot>>,
}

impl TemplateApp {
    pub fn new(bus: comm::Bus, jackit: jackit::JackIt, portbufs: Vec<portbuf::PortBuf>) -> Self {
        TemplateApp {
            jackit,
            portbufs,
            bus,
            plots: vec![],
        }
    }
}

impl eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let updates = &self.bus.updates(false);
        self.jackit.update(updates);
        for pb in &mut self.portbufs {
            pb.update(updates);
        }

	// plot controller
	for updt in updates {
	    match updt {
                comm::Update::Jack(comm::Jack::Connected {
                    connected,
                    port_names,
                }) => {
		    if *connected {
			self.plots = vec![Box::new(Scope::new(port_names.clone()))]
                    } else {
			self.plots = vec![];
		    }
		},
		_ => (),
	    }
	}
	// for plt in &mut self.plots {
	//     plt.update(updates)
	// }

        if cfg!(debug_assertions) {
            egui::SidePanel::right("Diagnostics")
                .show(ctx, |ui| diagnostics(ui, &self.portbufs, &self.jackit));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
	    for plt in &mut self.plots {
		plt.plot(ui, &self.portbufs);
	    }
	});
    }

    fn on_exit(&mut self) {
        self.portbufs.iter_mut().for_each(|pb| pb.quit());
        self.jackit.stop();
        std::thread::sleep(comm::PORT_BUF_WAIT_DUR);
    }
}

trait XPlot {
    fn plot(&mut self, ui: &mut egui::Ui, bufs: &Vec<portbuf::PortBuf>);
    fn update(&mut self, updts: &Vec<comm::Update>);
}

struct Scope {
    port_names: Vec<String>,
    sample_window: usize
}

impl Scope {
    fn new(port_names: Vec<String>) -> Self {
        Scope { port_names, sample_window: 1024 }
    }
}

impl XPlot for Scope {
    fn plot(&mut self, ui: &mut egui::Ui, portbufs: &Vec<portbuf::PortBuf>) {
	ui.add(egui::Slider::new(&mut self.sample_window, 64..=1024).text("My value"));
        let mut lines: Vec<Line> = self
            .port_names
            .iter()
            .filter_map(|port_name| portbufs.iter().find(|pb| &pb.name == port_name))
            .map(|pb| Line::new(PlotPoints::new(pb.raw_n(self.sample_window))))
            .collect();
        Plot::new("my_plot").view_aspect(2.0).show(ui, |plot_ui| {
            if lines.len() > 0 {
                plot_ui.line(lines.remove(0));
            }
        });
    }

    fn update(&mut self, updts: &Vec<comm::Update>) {
        for updt in updts {
            match updt {
                comm::Update::Jack(comm::Jack::Connected {
                    connected: _,
                    port_names,
                }) => {
                    self.port_names.push(
                        port_names
                            .first()
                            .expect("Scope.update port name update to contain port name")
                            .clone(),
                    );
                }
                _ => (),
            }
        }
    }
}

pub fn diagnostics(ui: &mut egui::Ui, portbufs: &Vec<portbuf::PortBuf>, jackit: &jackit::JackIt) {
    ui.heading("Port Connections");
    for portbuf::PortBuf { name, enabled, .. } in portbufs {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            ui.label(name);
            if *enabled {
                ui.label("☑");
            } else {
                ui.label("☐");
            }
        });
    }
    ui.separator();
    ui.heading("Jack Process Diagnostics");
    ui.label(format!(
        "Avg Process Time: {:?}",
        jackit.timing.avg_diag_cycle_time
    ));
    ui.label(format!(
        "Max Process Time: {:?}",
        jackit.timing.max_diag_cycle_time
    ));

    ui.heading("PortBuf Process Diagnostics");
    for portbuf::PortBuf { name, timing, .. } in portbufs {
        ui.label(name);
        ui.label(format!(
            "Avg Process Time: {:?}",
            timing.avg_diag_cycle_time
        ));
        ui.label(format!(
            "Max Process Time: {:?}",
            timing.max_diag_cycle_time
        ));
    }

    ui.heading("PortBuf Capacity");
    for portbuf in portbufs {
        ui.label(&portbuf.name);
        let (agg, raw) = portbuf.capacity();
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            ui.label("Agg: ");
            ui.add(egui::widgets::ProgressBar::new(
                agg as f32 / comm::PORT_BUF_SIZE as f32,
            ));
        });
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            ui.label("Raw: ");
            ui.add(egui::widgets::ProgressBar::new(
                raw as f32 / comm::PORT_BUF_SIZE as f32,
            ));
        });
    }
}
