use crate::jackit;
use egui::plot::{Line, Plot, PlotPoints};

const ARR_N: usize = 4194304;

struct ArrayView {
    idx: usize,
    arr: Vec<f32>,
}

impl ArrayView {
    fn push(&mut self, x: f32) {
	if self.idx == self.arr.len() {
	    self.idx = 0;
	}
	self.arr[self.idx] = x;
	self.idx += 1;
    }

    fn get_n(&mut self, n: usize) -> &[f32] {
	if n > self.idx {
	    &self.arr[0..self.idx]
	} else {
	    let start = self.idx - n;
	    &self.arr[start..self.idx]
	}
    }
}

pub struct TemplateApp {
    jackit: jackit::JackIt,
    rx: crossbeam_channel::Receiver<f32>,
    aview: ArrayView,
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(jackit: jackit::JackIt, rx: crossbeam_channel::Receiver<f32>) -> Self {
        TemplateApp {
	    jackit,
            rx,
	    aview: ArrayView{idx: 0, arr: Vec::with_capacity(ARR_N)},
        }
    }
}

impl  eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Self { rx, .. } = self;

	while !rx.is_empty() {
	    let res = rx.try_recv();
	    match res {
		Ok(x) => self.aview.push(x),
		Err(e) => eprintln!("Sucking RX didn't suck"),
	    }
	}

        egui::CentralPanel::default().show(ctx, |ui| {
            let line = Line::new(PlotPoints::from_ys_f32(self.aview.get_n(1024)));
            Plot::new("my_plot")
                .view_aspect(2.0)
                .show(ui, |plot_ui| plot_ui.line(line));
        });
    }

    fn on_close_event(&mut self) -> bool {
	self.jackit.quit();
	true
    }
}
