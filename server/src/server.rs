use crate::aerodrome::{
	Action, Aerodrome, AerodromeConnection, AerodromeInit, AerodromePilots,
	AerodromeUpdates, OpenAerodrome, Subscribe,
};
use crate::api::{ApiManager, UpdateToken};
use crate::client::Client;
use crate::config::ConfigManager;
use crate::connection::{
	ConnectionChanged, ConnectionManager, RequestConnection,
};
use crate::settings::Settings;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bars_ipc::{AerodromeConfig, ConnectionTarget, Downstream, Upstream};

use actix::{
	Actor, ActorContext, Addr, Arbiter, AsyncContext, Context, Handler, Message,
};
use actix_broker::BrokerSubscribe;
use tracing::warn;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_mins(5);

pub type ClientId = u64;

#[derive(Message)]
#[rtype(result = "()")]
pub struct IpcDownstream {
	pub message: Downstream,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct IpcUpstream {
	pub client: ClientId,
	pub message: Upstream,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AddClient {
	pub id: ClientId,
	pub addr: Addr<Client>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct RemoveClient {
	pub id: ClientId,
}

pub struct Server {
	api: Addr<ApiManager>,
	config: Addr<ConfigManager>,
	connection: Addr<ConnectionManager>,

	disconnection: Option<Instant>,
	network: ConnectionTarget,
	clients: HashMap<ClientId, Addr<Client>>,
	aerodromes: HashMap<String, AerodromeHandle>,
	settings: Settings,
}

struct AerodromeHandle {
	addr: Addr<Aerodrome>,
	config: Option<Arc<AerodromeConfig>>,
	pilots: Vec<String>,
}

impl Server {
	pub fn new() -> Addr<Self> {
		let settings = Settings::load()
			.inspect_err(|err| warn!("failed to load settings: {err}"))
			.unwrap_or_default();

		let api = ApiManager::new(&settings.api);
		let config = ConfigManager::new(api.clone());
		let connection = ConnectionManager::new(api.clone());

		let this = Self {
			api,
			config,
			connection,

			disconnection: Some(Instant::now()),
			network: ConnectionTarget::None,
			clients: HashMap::new(),
			aerodromes: HashMap::new(),
			settings,
		};
		Self::start_in_arbiter(&Arbiter::current(), |_ctx| this)
	}

	fn send(&self, client: ClientId, message: Downstream) {
		self
			.clients
			.get(&client)
			.inspect(|client| client.do_send(IpcDownstream { message }));
	}

	fn broadcast(&self, message: Downstream) {
		for client in self.clients.values() {
			client.do_send(IpcDownstream {
				message: message.clone(),
			});
		}
	}
}

impl Actor for Server {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		ctx.run_interval(Duration::from_secs(5), |this, ctx| {
			if let Some(disconnection) = this.disconnection {
				if disconnection.elapsed() > SHUTDOWN_TIMEOUT {
					ctx.stop();
				}
			}
		});

		self.subscribe_system_async::<ConnectionChanged>(ctx);
	}
}

impl Handler<IpcUpstream> for Server {
	type Result = ();

	fn handle(&mut self, msg: IpcUpstream, ctx: &mut Self::Context) {
		match msg.message {
			Upstream::Authenticate { token } => {
				if self.network == ConnectionTarget::None {
					self.settings.api.token = token.clone();
					self.api.do_send(UpdateToken(token));

					if let Err(err) = self.settings.save() {
						warn!("failed to save settings: {err}");
					}
				} else {
					self.send(
						msg.client,
						Downstream::UserMessage(
							"Please disconnect before re-authenticating.".into(),
						),
					);
				}
			},
			Upstream::Connect { target } => {
				self.connection.do_send(RequestConnection(target));
			},
			Upstream::Subscribe {
				aerodrome,
				subscribe,
			} => {
				self
					.aerodromes
					.entry(aerodrome.clone())
					.or_insert_with(|| {
						let addr = Aerodrome::new(AerodromeInit {
							icao: aerodrome,
							api: self.api.clone(),
							config: self.config.clone(),
							server: ctx.address(),
						});
						addr.do_send(ConnectionChanged(self.network));
						AerodromeHandle {
							addr,
							config: None,
							pilots: Vec::new(),
						}
					})
					.addr
					.do_send(Subscribe(subscribe));
			},
			Upstream::GraphAction { aerodrome, action } => {
				self
					.aerodromes
					.get(&aerodrome)
					.map(|aerodrome| aerodrome.addr.do_send(Action(action)));
			},
		}
	}
}

impl Handler<OpenAerodrome> for Server {
	type Result = ();

	fn handle(&mut self, message: OpenAerodrome, _ctx: &mut Self::Context) {
		self.broadcast(Downstream::OpenAerodrome {
			aerodrome: message.aerodrome.clone(),
			config: (*message.config).clone(),
		});
		self
			.aerodromes
			.get_mut(&message.aerodrome)
			.map(|aerodrome| aerodrome.config = Some(message.config));
	}
}

impl Handler<AerodromeConnection> for Server {
	type Result = ();

	fn handle(&mut self, message: AerodromeConnection, _ctx: &mut Self::Context) {
		self.broadcast(Downstream::AerodromeConnection {
			aerodrome: message.aerodrome,
			capacity: message.capacity,
		});
	}
}

impl Handler<AerodromeUpdates> for Server {
	type Result = ();

	fn handle(&mut self, message: AerodromeUpdates, _ctx: &mut Self::Context) {
		for update in message.updates {
			self.broadcast(Downstream::MapUpdate {
				aerodrome: message.aerodrome.clone(),
				update,
			});
		}
	}
}

impl Handler<AerodromePilots> for Server {
	type Result = ();

	fn handle(&mut self, message: AerodromePilots, _ctx: &mut Self::Context) {
		self
			.aerodromes
			.get_mut(&message.aerodrome)
			.map(|aerodrome| aerodrome.pilots = message.pilots);
		self.broadcast(Downstream::Pilots {
			callsigns: self
				.aerodromes
				.values()
				.flat_map(|handle| handle.pilots.iter())
				.cloned()
				.collect::<Vec<_>>(),
		});
	}
}

impl Handler<AddClient> for Server {
	type Result = ();

	fn handle(&mut self, message: AddClient, _ctx: &mut Self::Context) {
		self.disconnection = None;
		self.clients.insert(message.id, message.addr);

		self.send(message.id, Downstream::Hello);
		self.send(
			message.id,
			Downstream::Connection {
				target: self.network,
			},
		);
		for (icao, aerodrome) in &self.aerodromes {
			if let Some(config) = &aerodrome.config {
				self.send(
					message.id,
					Downstream::OpenAerodrome {
						aerodrome: icao.clone(),
						config: (**config).clone(),
					},
				);
			}
		}
	}
}

impl Handler<RemoveClient> for Server {
	type Result = ();

	fn handle(&mut self, msg: RemoveClient, _ctx: &mut Self::Context) {
		self.clients.remove(&msg.id);
		if self.clients.is_empty() {
			self.disconnection = Some(Instant::now());
		}
	}
}

impl Handler<ConnectionChanged> for Server {
	type Result = ();

	fn handle(
		&mut self,
		ConnectionChanged(target): ConnectionChanged,
		_ctx: &mut Self::Context,
	) {
		self.network = target;
		self.broadcast(Downstream::Connection { target });
	}
}
