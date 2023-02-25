use crate::comm;
use crate::jackit;
use crate::portbuf;

use egui::plot::{Line, Plot, PlotPoints};

pub struct TemplateApp {
    // sub-systems
    jackit: jackit::JackIt,
    portbufs: Vec<portbuf::PortBuf>,
    bus: comm::Bus,
}

impl TemplateApp {
    pub fn new(bus: comm::Bus, jackit: jackit::JackIt, portbufs: Vec<portbuf::PortBuf>) -> Self {
        TemplateApp {
            jackit,
            portbufs,
            bus,
        }
    }
}

impl eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let updates = self.bus.updates(false);
        self.jackit.update(&updates);
        for pb in &mut self.portbufs {
            pb.update(&updates);
        }

        if cfg!(debug_assertions) {
            egui::SidePanel::right("Diagnostics")
                .show(ctx, |ui| diagnostics(ui, &self.portbufs, &self.jackit));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // for (idx, PortState{name, ..}) in &self.state.ports {
            // 	let plots = self.plots.iter().filter(|plt| plt.has_port(name));
            // 	if plots.len() > 0 {

            // 	}
            // }
            // let line = Line::new(PlotPoints::from_ys_f32(self.aview.get_n(1024)));
            // Plot::new("my_plot")
            //     .view_aspect(2.0)
            //     .show(ui, |plot_ui| plot_ui.line(line));
        });
    }

    fn on_exit(&mut self) {
        self.portbufs.iter_mut().for_each(|pb| pb.quit());
        self.jackit.stop();
        std::thread::sleep(comm::PORT_BUF_WAIT_DUR);
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
