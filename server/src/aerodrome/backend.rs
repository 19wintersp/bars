use std::collections::HashMap;
use std::time::Duration;

use super::Aerodrome;
use crate::client::UserMessage;

use bars_graph::Patch;
use bars_ipc::ConnectionCapacity;
use bars_protocol::{ConnectionType, Downstream, StateUpdate, Upstream};

use actix::io::{SinkWrite, WriteHandler};
use actix::{
	Actor, ActorContext, Addr, Arbiter, AsyncContext, Context, Handler, Message,
	Running, StreamHandler,
};
use async_tungstenite::tokio::ConnectStream;
use async_tungstenite::{WebSocketSender, WebSocketStream, tungstenite};
use tracing::{debug, error, warn};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const PILOT_QUERY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Message)]
#[rtype(result = "()")]
pub struct CapacityChanged(pub ConnectionCapacity);

#[derive(Message)]
#[rtype(result = "()")]
pub struct PilotsChanged(pub Vec<String>);

#[derive(Message)]
#[rtype(result = "()")]
pub struct ApplyPatch(pub Patch);

#[derive(Message)]
#[rtype(result = "()")]
pub struct UpdateCrossing(pub String);

#[derive(Message)]
#[rtype(result = "()")]
pub struct UpdateScenery(pub HashMap<String, bool>);

pub struct Backend {
	aerodrome: Addr<Aerodrome>,
	sink: SinkWrite<tungstenite::Message, WebSocketSender<ConnectStream>>,
}

impl Backend {
	pub fn new(
		aerodrome: Addr<Aerodrome>,
		stream: WebSocketStream<ConnectStream>,
	) -> Addr<Self> {
		let (tx, rx) = stream.split();
		Self::start_in_arbiter(&Arbiter::current(), move |ctx| {
			ctx.add_stream(rx);
			Self {
				aerodrome,
				sink: SinkWrite::new(tx, ctx),
			}
		})
	}

	fn handle_message(
		&mut self,
		message: Downstream<Option<Patch>>,
		ctx: &mut Context<Self>,
	) {
		match message {
			Downstream::Heartbeat => self.send_message(&Upstream::HeartbeatAck),
			Downstream::HeartbeatAck => (),
			Downstream::Close => ctx.stop(),
			Downstream::Error { message } => self.aerodrome.do_send(UserMessage {
				aerodrome: None,
				message: format!("Server error: {message}"),
			}),
			Downstream::InitialState {
				connection_type,
				scenery: _,
				patch,
			} => {
				self
					.aerodrome
					.do_send(CapacityChanged(match connection_type {
						ConnectionType::Controller => ConnectionCapacity::Control,
						ConnectionType::Observer
						| ConnectionType::Pilot
						| ConnectionType::Other => ConnectionCapacity::Observe,
					}));
				self
					.aerodrome
					.do_send(ApplyPatch(patch.unwrap_or_default()));
			},
			Downstream::SharedStateUpdate {
				patch,
				controller_id: _,
			} => self
				.aerodrome
				.do_send(ApplyPatch(patch.unwrap_or_default())),
			Downstream::StopbarCrossing { object_id } => {
				self.aerodrome.do_send(UpdateCrossing(object_id))
			},
			Downstream::OnlinePilots { pilots } => self.aerodrome.do_send(
				PilotsChanged(pilots.into_iter().map(|pilot| pilot.callsign).collect()),
			),
			Downstream::ControllerConnect { .. }
			| Downstream::ControllerDisconnect { .. }
			| Downstream::StateUpdate { .. }
			| Downstream::StateSnapshot { .. }
			| Downstream::Other => (),
		}
	}

	fn send_message(&mut self, message: &Upstream<Patch>) {
		match serde_json::to_string(message) {
			Ok(message) => {
				let _ = self.sink.write(tungstenite::Message::Text(message.into()));
			},
			Err(err) => warn!("ignoring json serialisation error: {err}"),
		}
	}
}

impl Actor for Backend {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		self.send_message(&Upstream::GetOnlinePilots);

		ctx.run_interval(HEARTBEAT_INTERVAL, |this, _ctx| {
			this.send_message(&Upstream::Heartbeat);
		});

		ctx.run_interval(PILOT_QUERY_INTERVAL, |this, _ctx| {
			this.send_message(&Upstream::GetOnlinePilots);
		});

		ctx.run_interval(Duration::from_secs(1), |_this, ctx| {
			if !ctx.connected() {
				ctx.stop();
			}
		});
	}

	fn stopped(&mut self, _ctx: &mut Self::Context) {
		debug!("stopped backend");

		self.send_message(&Upstream::Close);
		self.sink.close();
		self
			.aerodrome
			.do_send(CapacityChanged(ConnectionCapacity::None));
	}
}

impl Handler<ApplyPatch> for Backend {
	type Result = ();

	fn handle(
		&mut self,
		ApplyPatch(patch): ApplyPatch,
		_ctx: &mut Self::Context,
	) {
		self.send_message(&Upstream::SharedStateUpdate { patch });
	}
}

impl Handler<UpdateScenery> for Backend {
	type Result = ();

	fn handle(
		&mut self,
		UpdateScenery(elements): UpdateScenery,
		_ctx: &mut Self::Context,
	) {
		self.send_message(&Upstream::MultiStateUpdate {
			updates: elements
				.into_iter()
				.map(|(object_id, state)| StateUpdate { object_id, state })
				.collect(),
		});
	}
}

impl StreamHandler<Result<tungstenite::Message, tungstenite::Error>>
	for Backend
{
	fn handle(
		&mut self,
		item: Result<tungstenite::Message, tungstenite::Error>,
		ctx: &mut Self::Context,
	) {
		match item {
			Ok(message) => match message {
				tungstenite::Message::Text(message) => {
					match serde_json::from_str::<Downstream<Option<Patch>>>(&message) {
						Ok(message) => self.handle_message(message, ctx),
						Err(err) => warn!("ignoring json deserialisation error: {err}"),
					}
				},
				_ => (),
			},
			Err(err) => {
				error!("tungstenite read error: {err}");
				ctx.stop();
			},
		}
	}
}

impl WriteHandler<tungstenite::Error> for Backend {
	fn error(
		&mut self,
		err: tungstenite::Error,
		_ctx: &mut Self::Context,
	) -> Running {
		error!("tungstenite write error: {err}");
		Running::Stop
	}
}
