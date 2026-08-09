use crate::actor::backend::{
	ApplyAction, Backend, BackendHandle, DispatchUpdates, GetInitialMapUpdates,
	SetPilots,
};
use crate::actor::client::{
	Client, ClientDownstream, ClientId, ClientUpstream, DispatchUserMessage,
};
use crate::actor::service::config::{ClearCache, ConfigService, GetConfig};
use crate::actor::service::settings::{SetApiToken, SettingsService};
use crate::actor::service::target::{SetTarget, TargetService};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bars_config::Icao;
use bars_ipc::{
	AerodromeConfig, AerodromeState, ConnectionTarget, Downstream, Upstream,
};

use actix::{
	Actor, ActorFutureExt, Addr, AsyncContext, Context, Handler, Message,
	Supervised, System, SystemService, WrapFuture,
};
use actix_broker::{BrokerIssue, BrokerSubscribe};
use tracing::{debug, warn};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_mins(1);

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

pub struct CoreService {
	disconnection: Option<Instant>,
	target: ConnectionTarget,
	clients: HashMap<ClientId, Addr<Client>>,
	aerodromes: HashMap<Icao, Aerodrome>,
}

#[derive(Default)]
struct Aerodrome {
	state: AerodromeState,
	rc: usize,
	config: Option<Arc<AerodromeConfig>>,
	backend: Option<BackendHandle>,
	pilots: Vec<String>,
}

impl CoreService {
	fn send(&self, client: ClientId, message: Downstream) {
		self
			.clients
			.get(&client)
			.inspect(|client| client.do_send(ClientDownstream { message }));
	}

	fn broadcast(&self, message: Downstream) {
		for client in self.clients.values() {
			client.do_send(ClientDownstream {
				message: message.clone(),
			});
		}
	}

	fn set_aerodrome_config(&mut self, icao: Icao, config: Arc<AerodromeConfig>) {
		if let Some(aerodrome) = self.aerodromes.get_mut(&icao) {
			let state = AerodromeState::None;
			let message = Downstream::Aerodrome {
				aerodrome: icao,
				config: Some((*config).clone()),
				state: Some(state),
				updates: Vec::new(),
			};

			aerodrome.config = Some(config.clone());
			aerodrome.state = state;

			self.start_aerodrome_backend(icao);
			self.broadcast(message);
		}
	}

	fn set_aerodrome_state(&mut self, icao: Icao, state: AerodromeState) {
		if let Some(aerodrome) = self.aerodromes.get_mut(&icao) {
			aerodrome.state = state;
			self.broadcast(Downstream::Aerodrome {
				aerodrome: icao,
				config: None,
				state: Some(state),
				updates: Vec::new(),
			});
		}
	}

	fn load_aerodrome_config(&mut self, icao: Icao, ctx: &mut Context<Self>) {
		if let Some(aerodrome) = self.aerodromes.get_mut(&icao)
			// check config not already loaded...
			&& aerodrome.config.is_none()
			// ...and not being loaded
			&& aerodrome.state != AerodromeState::Loading
		{
			ctx.spawn(
				ConfigService::from_registry()
					.send(GetConfig { aerodrome: icao })
					.into_actor(self)
					.map(move |res, this, _ctx| {
						match res.map_err(|err| err.into()).flatten() {
							Ok(config) => {
								debug!("config loaded for {icao}");

								this.set_aerodrome_config(icao, config);
							},
							Err(err) => {
								warn!("config load for {icao} failed: {err}");

								this.set_aerodrome_state(icao, AerodromeState::Error);
								this.issue_system_async(DispatchUserMessage {
									aerodrome: Some(icao),
									message: format!("Could not load config for {icao}: {err}"),
								});
							},
						}
					}),
			);

			self.set_aerodrome_state(icao, AerodromeState::Loading);
		}
	}

	fn start_aerodrome_backend(&mut self, icao: Icao) {
		if let Some(Aerodrome {
			rc: 1..,
			config: Some(config),
			backend,
			..
		}) = self.aerodromes.get_mut(&icao)
		{
			*backend = Some(BackendHandle(Backend::new(icao, config.clone())));
		}
	}
}

impl Default for CoreService {
	fn default() -> Self {
		Self {
			disconnection: Some(Instant::now()),
			target: ConnectionTarget::None,
			clients: HashMap::new(),
			aerodromes: HashMap::new(),
		}
	}
}

impl Actor for CoreService {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		ctx.run_interval(Duration::from_secs(5), |this, _ctx| {
			if this
				.disconnection
				.is_some_and(|ts| ts.elapsed() > SHUTDOWN_TIMEOUT)
			{
				debug!("interval elapsed, stopping");
				System::current().stop();
			}
		});

		self.subscribe_system_async::<SetTarget>(ctx);
	}
}

impl Supervised for CoreService {}

impl SystemService for CoreService {}

impl Handler<ClientUpstream> for CoreService {
	type Result = ();

	fn handle(&mut self, msg: ClientUpstream, ctx: &mut Self::Context) {
		match msg.message {
			Upstream::Authenticate { token } => {
				if self.target == ConnectionTarget::None {
					SettingsService::from_registry().do_send(SetApiToken(token));
				} else {
					self.send(
						msg.client,
						Downstream::UserMessage {
							message: "Please disconnect before re-authenticating.".into(),
						},
					);
				}
			},
			Upstream::Connect { target } => {
				TargetService::from_registry().do_send(SetTarget(target));
			},
			Upstream::Subscribe {
				aerodrome: icao,
				subscribe,
			} => {
				let aerodrome = self
					.aerodromes
					.entry(icao)
					.or_insert_with(|| Aerodrome::default());

				if subscribe {
					aerodrome.rc += 1;

					if aerodrome.rc == 1 {
						self.start_aerodrome_backend(icao);
					} else if let Some(backend) = &aerodrome.backend {
						ctx.spawn(
							backend.0.send(GetInitialMapUpdates).into_actor(self).map(
								move |res, this, _ctx| {
									if let Ok(updates) = res {
										this.send(
											msg.client,
											Downstream::Aerodrome {
												aerodrome: icao,
												config: None,
												state: None,
												updates: updates,
											},
										)
									}
								},
							),
						);
					}

					self.load_aerodrome_config(icao, ctx);
				} else {
					match aerodrome.rc {
						0 => warn!("rc underflow"),
						1 => {
							debug!("rc for {icao} zero, stopping");
							aerodrome.rc = 0;
							if aerodrome.backend.take().is_some() {
								self.set_aerodrome_state(icao, AerodromeState::None);
							}
						},
						2.. => aerodrome.rc -= 1,
					}
				}
			},
			Upstream::GraphAction { aerodrome, action } => {
				self
					.aerodromes
					.get(&aerodrome)
					.and_then(|aerodrome| aerodrome.backend.as_ref())
					.map(|backend| backend.0.do_send(ApplyAction(action)));
			},
			Upstream::Reload => {
				if self.aerodromes.values().all(|aerodrome| aerodrome.rc == 0) {
					self
						.aerodromes
						.values_mut()
						.for_each(|aerodrome| aerodrome.config = None);
					ConfigService::from_registry().do_send(ClearCache);
				} else {
					self.send(
						msg.client,
						Downstream::UserMessage {
							message: "Cannot reload with active aerodromes.".into(),
						},
					);
				}
			},
		}
	}
}

impl Handler<DispatchUpdates> for CoreService {
	type Result = ();

	fn handle(&mut self, message: DispatchUpdates, _ctx: &mut Self::Context) {
		if let Some(state) = message.state {
			self
				.aerodromes
				.get_mut(&message.aerodrome)
				.map(|aerodrome| aerodrome.state = state);
		}

		self.broadcast(Downstream::Aerodrome {
			aerodrome: message.aerodrome,
			config: None,
			state: message.state,
			updates: message.updates,
		});
	}
}

impl Handler<SetPilots> for CoreService {
	type Result = ();

	fn handle(&mut self, message: SetPilots, _ctx: &mut Self::Context) {
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

impl Handler<AddClient> for CoreService {
	type Result = ();

	fn handle(&mut self, message: AddClient, _ctx: &mut Self::Context) {
		self.disconnection = None;
		self.clients.insert(message.id, message.addr);

		self.send(message.id, Downstream::Hello);
		self.send(
			message.id,
			Downstream::Connection {
				target: self.target,
			},
		);
		for (icao, aerodrome) in &self.aerodromes {
			self.send(
				message.id,
				Downstream::Aerodrome {
					aerodrome: *icao,
					config: aerodrome.config.as_ref().map(|config| (**config).clone()),
					state: Some(aerodrome.state),
					updates: Vec::new(),
				},
			);
		}
	}
}

impl Handler<RemoveClient> for CoreService {
	type Result = ();

	fn handle(&mut self, msg: RemoveClient, _ctx: &mut Self::Context) {
		self.clients.remove(&msg.id);
		if self.clients.is_empty() {
			debug!("no clients connected, disconnecting soon");
			self.disconnection = Some(Instant::now());
		}
	}
}

impl Handler<SetTarget> for CoreService {
	type Result = ();

	fn handle(&mut self, SetTarget(target): SetTarget, ctx: &mut Self::Context) {
		self.target = target;
		self.broadcast(Downstream::Connection { target });

		if target != ConnectionTarget::None {
			for icao in self.aerodromes.keys().copied().collect::<Vec<_>>() {
				self.load_aerodrome_config(icao, ctx);
			}
		}
	}
}
