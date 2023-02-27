use crate::comm;
use crate::jackit;
use crate::portbuf;

use egui::plot::{Line, Plot, PlotBounds, PlotPoints};

pub struct TemplateApp<const N: usize> {
    // sub-systems
    jackit: jackit::JackIt,
    portbufs: Vec<portbuf::PortBuf<N>>,
    bus: comm::Bus,
    plots: Vec<Box<dyn XPlot<N>>>,
}

impl<const N: usize> TemplateApp<N> {
    pub fn new(bus: comm::Bus, jackit: jackit::JackIt, portbufs: Vec<portbuf::PortBuf<N>>) -> Self {
        TemplateApp {
            jackit,
            portbufs,
            bus,
            plots: vec![],
        }
    }
}

impl<const N: usize> eframe::App for TemplateApp<N> {
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
                }
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

trait XPlot<const N: usize> {
    fn plot(&mut self, ui: &mut egui::Ui, bufs: &Vec<portbuf::PortBuf<N>>);
    fn update(&mut self, updts: &Vec<comm::Update>);
}

struct Scope {
    port_names: Vec<String>,
    time_window: f64,
}

impl Scope {
    fn new(port_names: Vec<String>) -> Self {
        Scope {
            port_names,
            time_window: 0.0025, // 400hz sine wave period
        }
    }
}

impl<const N: usize> XPlot<N> for Scope {
    fn plot(&mut self, ui: &mut egui::Ui, portbufs: &Vec<portbuf::PortBuf<N>>) {
        ui.add(egui::Slider::new(&mut self.time_window, 5.0e-4..=0.05).text("My value"));
        let lines: Vec<Line> = self
            .port_names
            .iter()
            .filter_map(|port_name| portbufs.iter().find(|pb| &pb.name == port_name))
            .map(|pb| Line::new(PlotPoints::new(pb.time_window(self.time_window, 0.0))))
            .collect();
        Plot::new("my_plot").view_aspect(2.0).show(ui, |plot_ui| {
            lines.into_iter().for_each(|line| plot_ui.line(line));
            plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                [0.0, -1.1],
                [self.time_window, 1.1],
            ))
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

pub fn diagnostics<const N: usize>(
    ui: &mut egui::Ui,
    portbufs: &Vec<portbuf::PortBuf<N>>,
    jackit: &jackit::JackIt,
) {
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
