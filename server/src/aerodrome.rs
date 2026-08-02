mod backend;
mod graph;

use self::backend::{ApplyPatch, Backend};
use self::graph::Graph;
use crate::aerodrome::backend::{
	CapacityChanged, PilotsChanged, UpdateCrossing, UpdateScenery,
};
use crate::api::{ApiManager, CreateConnectRequest};
use crate::client::UserMessage;
use crate::config::{ConfigManager, GetConfig};
use crate::connection::ConnectionChanged;
use crate::server::Server;

use std::sync::Arc;
use std::time::Duration;

use bars_graph::MapUpdate;
use bars_ipc::{
	AerodromeConfig, ConnectionCapacity, ConnectionTarget, GraphAction,
};

use actix::{
	Actor, ActorFutureExt, Addr, Arbiter, AsyncContext, Context, Handler,
	Message, WrapFuture,
};
use actix_broker::{BrokerIssue, BrokerSubscribe};
use anyhow::Error;
use async_tungstenite::tokio::connect_async;
use tracing::warn;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Action(pub GraphAction);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Subscribe(pub bool);

#[derive(Message)]
#[rtype(result = "()")]
pub struct OpenAerodrome {
	pub aerodrome: String,
	pub config: Arc<AerodromeConfig>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AerodromeConnection {
	pub aerodrome: String,
	pub capacity: ConnectionCapacity,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AerodromeUpdates {
	pub aerodrome: String,
	pub updates: Vec<MapUpdate>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct AerodromePilots {
	pub aerodrome: String,
	pub pilots: Vec<String>,
}

pub struct Aerodrome {
	init: AerodromeInit,
	rc: usize,
	network: ConnectionTarget,
	capacity: ConnectionCapacity,
	config: Option<Arc<AerodromeConfig>>,
	graph: Option<Graph>,
	backend: Option<Addr<Backend>>,
}

pub struct AerodromeInit {
	pub icao: String,
	pub api: Addr<ApiManager>,
	pub config: Addr<ConfigManager>,
	pub server: Addr<Server>,
}

impl Aerodrome {
	pub fn new(init: AerodromeInit) -> Addr<Self> {
		Self::start_in_arbiter(&Arbiter::current(), |_ctx| Self {
			init,
			rc: 0,
			network: ConnectionTarget::None,
			capacity: ConnectionCapacity::None,
			config: None,
			graph: None,
			backend: None,
		})
	}

	fn report_error(&self, message: String) {
		self.issue_system_async(UserMessage {
			aerodrome: Some(self.init.icao.clone()),
			message,
		});
	}

	fn update_connection(&mut self, ctx: &mut Context<Self>) {
		match self.network {
			ConnectionTarget::None => {
				self.disconnect();
			},
			ConnectionTarget::Network => {
				if self.config.is_some() {
					if self.rc > 0 {
						self.connect_network(ctx);
					}
				} else {
					self.load_config(ctx);
				}
			},
		}
	}

	fn disconnect(&mut self) {
		self.backend.take();
		self.graph = None;
	}

	fn connect_network(&self, ctx: &mut Context<Self>) {
		let fut = self
			.init
			.api
			.send(CreateConnectRequest {
				aerodrome: self.init.icao.clone(),
			})
			.into_actor(self)
			.then(|res, this, _ctx| {
				async {
					match res.map_err::<Error, _>(|err| err.into()).flatten() {
						Ok(request) => {
							connect_async(request).await.map_err(|err| err.into())
						},
						Err(err) => Err(err),
					}
				}
				.into_actor(this)
			})
			.map(|res, this, ctx| match res {
				Ok((stream, _)) => {
					this.backend = Some(Backend::new(ctx.address(), stream));
				},
				Err(err) => {
					warn!("connection failed: {err}");
					this.report_error(format!("Could not connect to network: {err}"));
				},
			});

		ctx.wait(fut);
	}

	fn load_config(&self, ctx: &mut Context<Self>) {
		ctx.wait(
			self
				.init
				.config
				.send(GetConfig {
					aerodrome: self.init.icao.clone(),
				})
				.into_actor(self)
				.map(
					|res, this, _ctx| match res.map_err(|err| err.into()).flatten() {
						Ok(config) => {
							this.config = Some(config.clone());
							this.init.server.do_send(OpenAerodrome {
								aerodrome: this.init.icao.clone(),
								config,
							});
						},
						Err(err) => {
							warn!("config loading failed: {err}");
							this.report_error(format!(
								"Could not load config for {}: {err}",
								this.init.icao
							));
						},
					},
				),
		);
	}

	fn fast_update(&mut self) {
		if let Some(graph) = &mut self.graph {
			self.init.server.do_send(AerodromeUpdates {
				aerodrome: self.init.icao.clone(),
				updates: graph.take_map_updates(),
			});
		}
	}

	fn slow_update(&mut self) {
		if let (Some(graph), Some(backend)) = (&mut self.graph, &self.backend)
			&& self.capacity == ConnectionCapacity::Control
		{
			let (patch, scenery) = graph.tick();
			if let Some(patch) = patch {
				backend.do_send(ApplyPatch(patch));
			}
			if !scenery.is_empty() {
				backend.do_send(UpdateScenery(scenery));
			}
		}
	}
}

impl Actor for Aerodrome {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		self.subscribe_system_async::<ConnectionChanged>(ctx);
		self.load_config(ctx);

		ctx.run_interval(Duration::from_secs(1), |this, _ctx| {
			this.fast_update();
			this.slow_update();
		});
	}
}

impl Handler<Action> for Aerodrome {
	type Result = ();

	fn handle(&mut self, Action(action): Action, _ctx: &mut Self::Context) {
		if let Some(graph) = &mut self.graph
			&& self.capacity == ConnectionCapacity::Control
		{
			graph.apply_action(action);
		}
	}
}

impl Handler<Subscribe> for Aerodrome {
	type Result = ();

	fn handle(
		&mut self,
		Subscribe(subscribe): Subscribe,
		ctx: &mut Self::Context,
	) {
		if subscribe {
			self.rc += 1;

			if self.rc == 1 {
				self.update_connection(ctx);
			}

			if let Some(graph) = &self.graph {
				let updates = graph.initial_map_updates();
				if !updates.is_empty() {
					self.init.server.do_send(AerodromeUpdates {
						aerodrome: self.init.icao.clone(),
						updates,
					});
				}
			}
		} else {
			self.rc -= 1;

			if self.rc == 0 {
				self.disconnect();
			}
		}
	}
}

impl Handler<ApplyPatch> for Aerodrome {
	type Result = ();

	fn handle(
		&mut self,
		ApplyPatch(patch): ApplyPatch,
		_ctx: &mut Self::Context,
	) {
		if let Some(graph) = &mut self.graph {
			graph.apply_patch(&patch);
		} else if let Some(config) = &self.config {
			self.graph = Some(Graph::new_with_patch(config.clone(), patch));
		}
	}
}

impl Handler<CapacityChanged> for Aerodrome {
	type Result = ();

	fn handle(
		&mut self,
		CapacityChanged(capacity): CapacityChanged,
		_ctx: &mut Self::Context,
	) {
		self.capacity = capacity;
		self.init.server.do_send(AerodromeConnection {
			aerodrome: self.init.icao.clone(),
			capacity,
		});
	}
}

impl Handler<UpdateCrossing> for Aerodrome {
	type Result = ();

	fn handle(
		&mut self,
		UpdateCrossing(object): UpdateCrossing,
		_ctx: &mut Self::Context,
	) {
		if let Some(graph) = &mut self.graph {
			graph.update_crossing(&object);
		}
	}
}

impl Handler<UserMessage> for Aerodrome {
	type Result = ();

	fn handle(&mut self, message: UserMessage, _ctx: &mut Self::Context) {
		self.report_error(message.message);
	}
}

impl Handler<ConnectionChanged> for Aerodrome {
	type Result = ();

	fn handle(
		&mut self,
		ConnectionChanged(target): ConnectionChanged,
		ctx: &mut Self::Context,
	) {
		self.network = target;
		self.update_connection(ctx);
	}
}

impl Handler<PilotsChanged> for Aerodrome {
	type Result = ();

	fn handle(
		&mut self,
		PilotsChanged(pilots): PilotsChanged,
		_ctx: &mut Self::Context,
	) {
		self.init.server.do_send(AerodromePilots {
			aerodrome: self.init.icao.clone(),
			pilots,
		});
	}
}
