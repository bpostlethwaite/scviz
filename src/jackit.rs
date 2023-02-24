use crate::comm::{self, Jack, Update};
use crate::app;
use anyhow::{bail, Result};
use jack;
use ringbuf;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Instant;

pub struct JackItConfig {
    pub ringbuf_cycle_size: usize,
    pub bus: comm::Bus,
}

enum JackClient {
    Active(jack::AsyncClient<Notifications, JProcessor>),
    Passive(jack::Client),
}

pub struct JackIt {
    client: Option<JackClient>,
    atomics: Vec<Arc<AtomicBool>>,
    port_names: Vec<String>,
}

impl JackIt {
    pub fn new(name: &str, port_names: Vec<String>) -> JackIt {
        // Create client
        let (client, _status) =
            jack::Client::new(name, jack::ClientOptions::NO_START_SERVER).unwrap();

        // start all ports in a Paused state until a Jack Port Connection is made
        let atomics = (0..port_names.len())
            .map(|_| Arc::new(AtomicBool::new(true)))
            .collect();

        JackIt {
            client: Some(JackClient::Passive(client)),
            atomics,
            port_names,
        }
    }

    pub fn start(&mut self, config: JackItConfig) -> Result<Vec<comm::RingConsumer>> {
        let client = match self.client.take() {
            Some(JackClient::Passive(client)) => client,
            Some(JackClient::Active(_)) => bail!("JackIt is already Active"),
            None => bail!("JackIt has no configured client"),
        };

        let mut rb_prods = vec![];
        let mut rb_cons = vec![];

        for _ in 0..self.port_names.len() {
            let ring_buf = ringbuf::HeapRb::<[f32; 2]>::new(
                (client.buffer_size() as usize) * config.ringbuf_cycle_size,
            );
            let (prod, cons) = ring_buf.split();
            rb_prods.push(prod);
            rb_cons.push(cons);
        }

        // Register ports. They will be used in a callback that will be
        // called when new data is available.
        let ports: Vec<jack::Port<jack::AudioIn>> = self
            .port_names
            .iter()
            .map(|pname| {
                client
                    .register_port(pname, jack::AudioIn::default())
                    .expect("JackIt port to successfully register")
            })
            .collect();

        // consume ringbufs and cloned atomics and ports
        let port_procs = ports
            .into_iter()
            .zip(rb_prods.into_iter().zip(self.atomics.clone().into_iter()))
            .map(|(port, (rb, pause))| PortProc { port, rb, pause })
            .collect();

        let jproc = JProcessor { bus: config.bus.clone(), port_procs };

        // Activate the client, which starts the processing.
        self.client = Some(JackClient::Active(
            client
                .activate_async(Notifications { bus: config.bus }, jproc)
                .expect("JackIt.start client.activate_async() to succeed"),
        ));

        Ok(rb_cons)
    }

    pub fn stop(&mut self) {
        match self.client.take() {
            Some(JackClient::Active(ac)) => match ac.deactivate() {
                Ok((client, ..)) => self.client = Some(JackClient::Passive(client)),
                Err(_) => panic!("Jack Client in bad state, no recoverable process implemented"),
            },
            Some(JackClient::Passive(_)) => (),
            None => (),
        }
    }

    pub fn port_names(&self) -> Vec<String> {
	self.port_names.clone()
    }

    pub fn buffer_size(&self) -> u32 {
        let client = match &self.client {
            Some(JackClient::Passive(c)) => c,
            Some(JackClient::Active(a)) => a.as_client(),
            None => panic!("JackIt has no configured client"),
        };
        client.buffer_size()
    }

    pub fn sample_rate(&self) -> usize {
        let client = match &self.client {
            Some(JackClient::Passive(c)) => c,
            Some(JackClient::Active(a)) => a.as_client(),
            None => panic!("JackIt has no configured client"),
        };
        client.sample_rate()
    }

    pub fn update(&mut self, updts: Vec<Update>, state: &mut app::State) {
        for updt in updts {
	    match updt {
		Update::Jack(Jack::Connected {
                    connected,
                    port_names,
		}) => {
		    for port_name in port_names.into_iter() {
			if let Some(idx) = self.port_names.iter().position(|n| port_name == *n) {
			    self.atomics
				.get(idx)
				.expect("jackit pause_port atomics to have idx of matched port")
				.store(connected, std::sync::atomic::Ordering::Relaxed);

			    state.ports_enabled[idx] = connected;
			} else {
			    eprintln!("Error: JackIt attempted to locate port {port_name} in {:?}", self.port_names);
			}
		    };
		},
		Update::Jack(Jack::ProcessTime(dur)) => state.jack_process_time = dur,
		_ => (),
            }
	}
    }
}

struct PortProc {
    port: jack::Port<jack::AudioIn>,
    rb: comm::RingProducer,
    pause: Arc<AtomicBool>,
}

struct JProcessor {
    port_procs: Vec<PortProc>,
    bus: comm::Bus,
}

impl jack::ProcessHandler for JProcessor {
    fn process(&mut self, _: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
	let now = Instant::now();

        let frame_time = ps.last_frame_time();
        let n_frames = ps.n_frames();
        let fs = (0..n_frames).map(|i| frame_time + i);

        self.port_procs
            .iter_mut()
            .filter(|pp| {
                // don't process any ports set to pause
                !pp.pause.load(std::sync::atomic::Ordering::Relaxed)
            })
            .for_each(|pp| {
                for (x, t) in std::iter::zip(pp.port.as_slice(ps), fs.clone()) {
                    match pp.rb.push([*x, t as f32]) {
                        Ok(()) => (),
                        Err(_) => panic!("Could not push to RingBuffer, buffer full"),
                    }
                }
            });

	let elapsed = now.elapsed();
	self.bus.send(Update::Jack(Jack::ProcessTime(elapsed)));
        jack::Control::Continue
    }
}

struct Notifications {
    bus: comm::Bus,
}

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
        client: &jack::Client,
        port_id_a: jack::PortId,
        port_id_b: jack::PortId,
        are_connected: bool,
    ) {
        let port_names: Vec<String> = vec![port_id_a, port_id_b]
            .iter()
            .filter_map(|&id| client.port_by_id(id))
            .filter(|p| client.is_mine(p))
            .filter_map(|p| p.name().ok())
            .collect();

        if port_names.len() > 0 {
            self.bus.send(Update::Jack(Jack::Connected {
                connected: are_connected,
                port_names,
            }))
        }

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
