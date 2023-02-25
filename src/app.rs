use crate::comm::{self, Update};
use crate::jackit;
use crate::portbuf;

use egui::plot::{Line, Plot, PlotPoints};

pub struct State {
    pub ports: Vec<PortState>,
    pub jack_process_diagnostics: comm::TimingDiagnostics,
}

impl State {
    fn new(port_names: Vec<String>) -> State {
        State {
            jack_process_diagnostics: comm::TimingDiagnostics::new(0),
            ports: port_names.into_iter().map(|n| PortState::new(n)).collect(),
        }
    }
}

pub struct PortState {
    pub name: String,
    pub timing: comm::TimingDiagnostics,
    pub enabled: bool,
}

impl PortState {
    fn new(name: String) -> Self {
        PortState {
            name,
            enabled: false,
            timing: comm::TimingDiagnostics::new(0),
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
        let state = State::new(jackit.port_names());
        TemplateApp {
            jackit,
            port_bufs,
            bus,
            state,
        }
    }
}

impl eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self { state, .. } = self;

        let updates = self.bus.updates(false);
        self.jackit.update(&updates, state);
        for pb in &self.port_bufs {
            pb.update(&updates, state);
        }
        egui::SidePanel::right("Diagnostics").show(ctx, |ui| {
            ui.heading("Port Connections");
            for port_state in &state.ports {
                let PortState { name, enabled, .. } = port_state;
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
                state.jack_process_diagnostics.avg_diag_cycle_time
            ));
            ui.label(format!(
                "Max Process Time: {:?}",
                state.jack_process_diagnostics.max_diag_cycle_time
            ));

            ui.heading("PortBuf Process Diagnostics");
            for port_state in &state.ports {
                let PortState { name, timing, .. } = port_state;
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
            for (port_buf, PortState { name, .. }) in std::iter::zip(&self.port_bufs, &state.ports)
            {
                ui.label(name);
                let (agg, raw) = port_buf.capacity();
		ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.label("Agg: ");
                    ui.add(egui::widgets::ProgressBar::new(agg as f32 / comm::PORT_BUF_SIZE as f32));
		});
		ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.label("Raw: ");
                    ui.add(egui::widgets::ProgressBar::new(raw as f32 / comm::PORT_BUF_SIZE as f32));
		});
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // let line = Line::new(PlotPoints::from_ys_f32(self.aview.get_n(1024)));
            // Plot::new("my_plot")
            //     .view_aspect(2.0)
            //     .show(ui, |plot_ui| plot_ui.line(line));
        });
    }

    fn on_exit(&mut self) {
        self.port_bufs.iter_mut().for_each(|pb| pb.quit());
        self.jackit.stop();
        std::thread::sleep(comm::PORT_BUF_WAIT_DUR);
    }
}
