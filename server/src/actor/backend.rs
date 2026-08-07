mod graph;
mod local;
mod network;

use crate::actor::backend::graph::Graph;
use crate::actor::backend::local::Local;
use crate::actor::backend::network::Network;
use crate::actor::client::DispatchUserMessage;
use crate::actor::service::core::CoreService;
use crate::actor::service::target::SetTarget;

use std::sync::Arc;
use std::time::Duration;

use bars_config::Icao;
use bars_graph::{MapUpdate, Patch};
use bars_ipc::{
	AerodromeConfig, AerodromeState, ConnectionTarget, GraphAction,
};
use bars_protocol::{ConnectionType, Downstream, StateUpdate, Upstream};

use actix::{
	Actor, ActorContext, Addr, AsyncContext, Context, Handler, Message,
	MessageResult, Recipient, SystemService,
};
use actix_broker::{BrokerIssue, BrokerSubscribe};
use tracing::{debug, trace, warn};

const CONNECTION_ATTEMPTS: usize = 3;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const PILOT_QUERY_INTERVAL: Duration = Duration::from_secs(30);
const STATE_UPDATE_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Close;

#[derive(Message)]
#[rtype(result = "()")]
pub struct ApplyAction(pub GraphAction);

#[derive(Message)]
#[rtype(result = "Vec<MapUpdate>")]
pub struct GetInitialMapUpdates;

#[derive(Message)]
#[rtype(result = "()")]
pub struct DispatchUpdates {
	pub aerodrome: Icao,
	pub state: Option<AerodromeState>,
	pub updates: Vec<MapUpdate>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetPilots {
	pub aerodrome: Icao,
	pub pilots: Vec<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct BackendDownstream(Downstream<Option<Patch>>);

#[derive(Message)]
#[rtype(result = "()")]
struct BackendUpstream(Upstream<Patch>);

#[derive(Message)]
#[rtype(result = "()")]
enum ConnectionState {
	Error,
	Connected,
}

pub struct Backend {
	icao: Icao,
	config: Arc<AerodromeConfig>,
	state: BackendState,
	target: ConnectionTarget,
	graph: Option<Graph>,
	connection: Option<Recipient<BackendUpstream>>,
}

#[derive(Debug)]
enum BackendState {
	None,
	Connecting(usize),
	Error,
	Connected,
}

impl Backend {
	pub fn new(icao: Icao, config: Arc<AerodromeConfig>) -> Addr<Self> {
		Self::create(move |_ctx| Self {
			icao,
			config,
			state: BackendState::None,
			target: ConnectionTarget::None,
			graph: None,
			connection: None,
		})
	}

	fn send(&self, message: Upstream<Patch>) {
		self
			.connection
			.as_ref()
			.map(|backend| backend.do_send(BackendUpstream(message)));
	}

	fn dispatch_state(&self) {
		CoreService::from_registry().do_send(DispatchUpdates {
			aerodrome: self.icao,
			state: Some(match self.state {
				BackendState::None => AerodromeState::None,
				BackendState::Connecting(_) => AerodromeState::Loading,
				BackendState::Error => AerodromeState::Error,
				BackendState::Connected => return,
			}),
			updates: Vec::new(),
		});
	}

	fn update_connection(&mut self, ctx: &mut Context<Self>) {
		if let BackendState::Connecting(attempt) = self.state {
			let addr = ctx.address().downgrade();
			self.connection = Some(match self.target {
				ConnectionTarget::None => {
					warn!("assertion failed: target is None but state is Connecting");
					return
				},
				ConnectionTarget::Network => Network::new(
					self.icao,
					addr,
					(attempt > 0)
						.then_some(CONNECTION_TIMEOUT)
						.unwrap_or_default(),
				)
				.recipient(),
				ConnectionTarget::Local => Local::new(addr).recipient(),
			});
		}
	}

	fn fast_update(&mut self) {
		if let Some(graph) = &mut self.graph {
			let updates = graph.take_map_updates();
			if !updates.is_empty() {
				CoreService::from_registry().do_send(DispatchUpdates {
					aerodrome: self.icao,
					state: None,
					updates,
				});
			}
		}
	}

	fn slow_update(&mut self) {
		if let Some(graph) = &mut self.graph {
			let (patch, scenery) = graph.tick();
			if let Some(patch) = patch {
				self.send(Upstream::SharedStateUpdate { patch });
			}
			if !scenery.is_empty() {
				self.send(Upstream::MultiStateUpdate {
					updates: scenery
						.into_iter()
						.map(|(object_id, state)| StateUpdate { object_id, state })
						.collect(),
				});
			}
		}
	}
}

impl Actor for Backend {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		debug!("backend start: {}", self.icao);

		self.subscribe_system_sync::<SetTarget>(ctx);

		ctx.run_interval(HEARTBEAT_INTERVAL, |this, _ctx| {
			this.send(Upstream::Heartbeat);
		});

		ctx.run_interval(PILOT_QUERY_INTERVAL, |this, _ctx| {
			this.send(Upstream::GetOnlinePilots);
		});

		ctx.run_interval(STATE_UPDATE_INTERVAL, |this, _ctx| {
			this.fast_update();
			this.slow_update();
		});

		ctx.run_interval(Duration::from_secs(1), |_this, ctx| {
			if !ctx.connected() {
				trace!("stop due disconnect");
				ctx.stop();
			}
		});
	}

	fn stopped(&mut self, _ctx: &mut Self::Context) {
		debug!("backend stop: {}", self.icao);

		self.send(Upstream::Close);
	}
}

impl Handler<Close> for Backend {
	type Result = ();

	fn handle(&mut self, _: Close, ctx: &mut Self::Context) {
		trace!("stop due message");
		ctx.stop();
	}
}

impl Handler<ApplyAction> for Backend {
	type Result = ();

	fn handle(
		&mut self,
		ApplyAction(action): ApplyAction,
		_ctx: &mut Self::Context,
	) {
		self.graph.as_mut().map(|graph| graph.apply_action(action));
		self.fast_update();
	}
}

impl Handler<GetInitialMapUpdates> for Backend {
	type Result = MessageResult<GetInitialMapUpdates>;

	fn handle(
		&mut self,
		_: GetInitialMapUpdates,
		_ctx: &mut Self::Context,
	) -> Self::Result {
		MessageResult(
			self
				.graph
				.as_ref()
				.map(|graph| graph.initial_map_updates())
				.unwrap_or_default(),
		)
	}
}

impl Handler<SetTarget> for Backend {
	type Result = ();

	fn handle(&mut self, SetTarget(target): SetTarget, ctx: &mut Self::Context) {
		self.target = target;
		self.graph.take();
		self.connection.take();

		if target == ConnectionTarget::None {
			self.state = BackendState::None;
		} else {
			self.state = BackendState::Connecting(0);
		}

		self.dispatch_state();
		self.update_connection(ctx);
	}
}

impl Handler<BackendDownstream> for Backend {
	type Result = ();

	fn handle(
		&mut self,
		BackendDownstream(message): BackendDownstream,
		ctx: &mut Self::Context,
	) {
		match message {
			Downstream::Heartbeat => self.send(Upstream::HeartbeatAck),
			Downstream::HeartbeatAck => (),
			Downstream::Close => {
				self.handle(ConnectionState::Error, ctx);
			},
			Downstream::Error { message } => {
				warn!("server error for {}: {message}", self.icao);
				self.issue_system_async(DispatchUserMessage {
					aerodrome: Some(self.icao),
					message: format!("Server error ({}): {message}", self.icao),
				});
			},
			Downstream::InitialState {
				connection_type,
				scenery: _,
				patch,
			} => {
				CoreService::from_registry().do_send(DispatchUpdates {
					aerodrome: self.icao,
					state: Some(match connection_type {
						ConnectionType::Controller => AerodromeState::Control,
						ConnectionType::Observer
						| ConnectionType::Pilot
						| ConnectionType::Other => AerodromeState::Observe,
					}),
					updates: Vec::new(),
				});

				self.graph = Some(Graph::new_with_patch(
					self.config.clone(),
					patch.unwrap_or_default(),
					connection_type == ConnectionType::Controller,
				));
				self.fast_update();
			},
			Downstream::SharedStateUpdate {
				patch,
				controller_id: _,
			} => {
				patch
					.zip(self.graph.as_mut())
					.map(|(patch, graph)| graph.apply_patch(&patch));
				self.fast_update();
			},
			Downstream::StopbarCrossing { object_id } => {
				self
					.graph
					.as_mut()
					.map(|graph| graph.update_crossing(&object_id));
				self.fast_update();
			},
			Downstream::OnlinePilots { pilots } => {
				CoreService::from_registry().do_send(SetPilots {
					aerodrome: self.icao,
					pilots: pilots.into_iter().map(|pilot| pilot.callsign).collect(),
				});
			},
			Downstream::ControllerConnect { .. }
			| Downstream::ControllerDisconnect { .. }
			| Downstream::StateUpdate { .. }
			| Downstream::StateSnapshot { .. }
			| Downstream::Other => (),
		}
	}
}

impl Handler<ConnectionState> for Backend {
	type Result = ();

	fn handle(&mut self, state: ConnectionState, ctx: &mut Self::Context) {
		if !ctx.connected() {
			return
		}

		match state {
			ConnectionState::Error => {
				self.state = match self.state {
					BackendState::None => BackendState::None,
					BackendState::Connecting(attempt) => {
						if attempt + 1 < CONNECTION_ATTEMPTS {
							BackendState::Connecting(attempt + 1)
						} else {
							BackendState::Error
						}
					},
					BackendState::Connected | BackendState::Error => BackendState::Error,
				};
				self.graph.take();
				self.connection.take();

				self.dispatch_state();
				self.update_connection(ctx);
			},
			ConnectionState::Connected => {
				self.state = BackendState::Connected;
				self.dispatch_state();
			},
		}
	}
}

pub struct BackendHandle(pub Addr<Backend>);

impl Drop for BackendHandle {
	fn drop(&mut self) {
		self.0.do_send(Close);
	}
}
