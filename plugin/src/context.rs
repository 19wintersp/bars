mod aerodrome;
mod client;

pub use self::aerodrome::Aerodrome;
use self::client::Client;

use std::collections::{HashMap, HashSet};
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};

use bars_config::Icao;
use bars_ipc::{ConnectionTarget, Downstream, Upstream};

use tracing::{info, warn};

pub struct Context {
	client: Client,
	client_state: ClientState,
	network_state: ConnectionTarget,
	subscriptions: HashMap<Icao, usize>,
	pilots: HashSet<String>,
	aerodromes: HashMap<Icao, Aerodrome>,
}

impl Context {
	pub fn new() -> Self {
		Self {
			client: Client::new(),
			client_state: ClientState::Connecting,
			network_state: ConnectionTarget::None,
			subscriptions: HashMap::new(),
			pilots: HashSet::new(),
			aerodromes: HashMap::new(),
		}
	}

	pub fn connect(&mut self) {
		info!("client reconnect");
		self.client.connect();
		self.client_state = ClientState::Connecting;
	}

	pub fn authenticate(&self, token: Option<&str>) {
		self.client.send(Upstream::Authenticate {
			token: token.map(|s| s.into()),
		});
	}

	pub fn reload(&self) {
		self.client.send(Upstream::Reload);
	}

	pub fn connect_network(&self, capacity: ConnectionTarget) {
		self.client.send(Upstream::Connect { target: capacity });
	}

	pub fn subscribe(&mut self, aerodrome: Icao, subscribe: bool) {
		let rc = self
			.subscriptions
			.entry(aerodrome)
			.and_modify(|rc| {
				if subscribe {
					*rc += 1;
				} else if *rc > 0 {
					*rc -= 1;
				}
			})
			.or_insert_with(|| {
				if subscribe {
					self.client.send(Upstream::Subscribe {
						aerodrome,
						subscribe: true,
					});
					1
				} else {
					0
				}
			});

		if *rc == 0 {
			self.client.send(Upstream::Subscribe {
				aerodrome,
				subscribe: false,
			});
			self.subscriptions.remove(&aerodrome);
		}
	}

	pub fn network_state(&self) -> ConnectionTarget {
		self.network_state
	}

	pub fn is_pilot_connected(&self, callsign: &str) -> bool {
		self.pilots.contains(callsign)
	}

	pub fn aerodrome(&self, icao: Icao) -> Option<&Aerodrome> {
		self.aerodromes.get(&icao)
	}

	pub fn aerodrome_mut(&mut self, icao: Icao) -> Option<AerodromeMut<'_>> {
		self
			.aerodromes
			.remove_entry(&icao)
			.map(|entry| AerodromeMut {
				context: self,
				entry: ManuallyDrop::new(entry),
			})
	}

	pub fn tick(&mut self) -> ContextState {
		let connected = self.client.is_connected();
		let mut user_messages = Vec::new();

		if connected {
			let messages = self.client.queue().collect::<Vec<_>>();
			for message in messages {
				match message {
					Downstream::Hello => {
						self.reset();

						for aerodrome in self.subscriptions.keys().copied() {
							self.client.send(Upstream::Subscribe {
								aerodrome,
								subscribe: true,
							});
						}
					},
					Downstream::UserMessage { message } => user_messages.push(message),
					Downstream::Connection { target } => {
						self.network_state = target;
					},
					Downstream::Aerodrome {
						aerodrome,
						config,
						state: capacity,
						updates,
					} => {
						if let Some(config) = config {
							self.aerodromes.insert(aerodrome, Aerodrome::new(config));
						}

						if let Some(aerodrome) = self.aerodromes.get_mut(&aerodrome) {
							if let Some(capacity) = capacity {
								aerodrome.set_capacity(capacity);
							}

							for update in updates {
								aerodrome.update(update);
							}
						} else {
							warn!("unhandled message for closed ad {aerodrome}");
						}
					},
					Downstream::Pilots { callsigns } => {
						self.pilots = HashSet::from_iter(callsigns);
					},
				}
			}
		}

		self.client_state = match (self.client_state, connected) {
			(_, true) => ClientState::Connected,
			(ClientState::Connected | ClientState::Connecting, false) => {
				warn!("client disconnection occurred");
				self.reset();
				ClientState::Disconnected
			},
			(_, false) => ClientState::Idle,
		};

		ContextState {
			user_messages,
			client_state: self.client_state,
		}
	}

	fn reset(&mut self) {
		self.network_state = ConnectionTarget::None;
		self.pilots.clear();
		self.aerodromes.clear();
	}
}

pub struct AerodromeMut<'a> {
	context: &'a mut Context,
	entry: ManuallyDrop<(Icao, Aerodrome)>,
}

impl Deref for AerodromeMut<'_> {
	type Target = Aerodrome;

	fn deref(&self) -> &Self::Target {
		&self.entry.1
	}
}

impl DerefMut for AerodromeMut<'_> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.entry.1
	}
}

impl Drop for AerodromeMut<'_> {
	fn drop(&mut self) {
		let (icao, mut aerodrome) = unsafe { ManuallyDrop::take(&mut self.entry) };

		for action in aerodrome.take_actions() {
			self.context.client.send(Upstream::GraphAction {
				aerodrome: icao,
				action,
			});
		}

		self.context.aerodromes.insert(icao, aerodrome);
	}
}

pub struct ContextState {
	pub user_messages: Vec<String>,
	pub client_state: ClientState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClientState {
	Idle,
	Connecting,
	Connected,
	Disconnected,
}
