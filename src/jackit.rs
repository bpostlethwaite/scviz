use crossbeam_channel::Sender;
use egui;
use jack;

// pub const AUDIO_BUFF_SIZE: usize = 8192;
// pub const FFT_MAX_SIZE: usize = 8192;
// pub const FFT_MAX_BUFF_SIZE: usize = 4097;
// pub const MAX_DATA_LENGTH: usize = 10000;

// For implementing FreqResponse and Freq see
// https://github.com/supercollider/supercollider/blob/develop/SCClassLibrary/Common/GUI/PlusGUI/Control/FreqScope.sc

pub struct JackIt {
    active_client: Option<jack::AsyncClient<Notifications, JProcessor>>,
}

impl JackIt {
    pub fn new(ctx: egui::Context, tx: Sender<f32>) -> JackIt {
        // Create client
        let (client, _status) =
            jack::Client::new("scmon", jack::ClientOptions::NO_START_SERVER).unwrap();

        // Register ports. They will be used in a callback that will be
        // called when new data is available.
        let port_1 = client
            .register_port("rust_in_l", jack::AudioIn::default())
            .unwrap();
        let port_2 = client
            .register_port("rust_in_r", jack::AudioIn::default())
            .unwrap();

        let sample_rate = client.sample_rate();
        let buffer_size = client.buffer_size();
        println!(
            "sample_rate = {}  buffer_size = {}",
            sample_rate, buffer_size
        );

        let jproc = JProcessor {
            tx,
            port_1,
            port_2,
            ctx,
        };

        // Activate the client, which starts the processing.
        let active_client = client.activate_async(Notifications, jproc).unwrap();

        //
        JackIt { active_client: Some(active_client) }
    }

    pub fn quit(&mut self) {
        match self.active_client.take() {
	    Some(ac) => {
		ac.deactivate().unwrap();
	    }
	    None => (),
	}
    }
}

struct JProcessor {
    port: jack::Port<jack::AudioIn>,
    port_2: jack::Port<jack::AudioIn>,
    tx: crossbeam_channel::Sender<f32>,
    ctx: egui::Context,
}

impl jack::ProcessHandler for JProcessor {
    fn process(&mut self, _: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        let in_a_p = self.port_1.as_slice(ps);
        let in_b_p = self.port_2.as_slice(ps);
        let sum_a: f32 = in_a_p.iter().sum();
        let avg_a = sum_a / in_a_p.len() as f32;
        if sum_a != 0.0 {
            self.tx.send(avg_a);
            self.ctx.request_repaint();
        }
        jack::Control::Continue
    }
}

struct Notifications;

impl jack::NotificationHandler for Notifications {
    fn thread_init(&self, _: &jack::Client) {
        println!("JACK: thread init");
    }

    fn shutdown(&mut self, status: jack::ClientStatus, reason: &str) {
        println!("JACK: shutdown with status {status:?} because \"{reason}\"",);
    }

    fn freewheel(&mut self, _: &jack::Client, is_enabled: bool) {
        println!(
            "JACK: freewheel mode is {}",
            if is_enabled { "on" } else { "off" }
        );
    }

    fn sample_rate(&mut self, _: &jack::Client, srate: jack::Frames) -> jack::Control {
        println!("JACK: sample rate changed to {srate}");
        jack::Control::Continue
    }

    fn client_registration(&mut self, _: &jack::Client, name: &str, is_reg: bool) {
        println!(
            "JACK: {} client with name \"{}\"",
            if is_reg { "registered" } else { "unregistered" },
            name
        );
    }

    fn port_registration(&mut self, _: &jack::Client, port_id: jack::PortId, is_reg: bool) {
        println!(
            "JACK: {} port with id {}",
            if is_reg { "registered" } else { "unregistered" },
            port_id
        );
    }

    fn port_rename(
        &mut self,
        _: &jack::Client,
        port_id: jack::PortId,
        old_name: &str,
        new_name: &str,
    ) -> jack::Control {
        println!("JACK: port with id {port_id} renamed from {old_name} to {new_name}",);
        jack::Control::Continue
    }

    fn ports_connected(
        &mut self,
        _: &jack::Client,
        port_id_a: jack::PortId,
        port_id_b: jack::PortId,
        are_connected: bool,
    ) {
        println!(
            "JACK: ports with id {} and {} are {}",
            port_id_a,
            port_id_b,
            if are_connected {
                "connected"
            } else {
                "disconnected"
            }
        );
    }

    fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
        println!("JACK: graph reordered");
        jack::Control::Continue
    }

    fn xrun(&mut self, _: &jack::Client) -> jack::Control {
        println!("JACK: xrun occurred");
        jack::Control::Continue
    }
}
