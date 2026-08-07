use std::time::Duration;

use crate::actor::backend::{
	Backend, BackendDownstream, BackendUpstream, ConnectionState,
};
use crate::actor::client::DispatchUserMessage;
use crate::actor::service::api::{ApiService, CreateConnectRequest};

use bars_config::Icao;
use bars_graph::Patch;
use bars_protocol::{Downstream, Upstream};

use actix::io::{SinkWrite, WriteHandler};
use actix::{
	Actor, ActorContext, ActorFutureExt, Addr, AsyncContext, Context, Handler,
	Running, StreamHandler, SystemService, WeakAddr, WrapFuture,
};
use actix_broker::BrokerIssue;
use anyhow::Error;
use async_tungstenite::tokio::{ConnectStream, connect_async};
use async_tungstenite::{WebSocketSender, tungstenite};
use tracing::{debug, trace, warn};

pub struct Network {
	icao: Icao,
	backend: WeakAddr<Backend>,
	delay: Duration,
	sink: Option<SinkWrite<tungstenite::Message, WebSocketSender<ConnectStream>>>,
}

impl Network {
	pub fn new(
		icao: Icao,
		backend: WeakAddr<Backend>,
		delay: Duration,
	) -> Addr<Self> {
		Self::create(move |_ctx| Self {
			icao,
			backend,
			delay,
			sink: None,
		})
	}

	fn fail(&self, ctx: &mut Context<Self>) {
		self
			.backend
			.upgrade()
			.map(|backend| backend.do_send(ConnectionState::Error));
		ctx.stop();
	}
}

impl Actor for Network {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		ctx.run_interval(Duration::from_secs(1), |_this, ctx| {
			if !ctx.connected() {
				ctx.stop();
			}
		});

		ctx.spawn(
			ApiService::from_registry()
				.send(CreateConnectRequest {
					aerodrome: self.icao,
				})
				.into_actor(self)
				.then(|res, this, _ctx| {
					let delay = this.delay;
					async move {
						match res.map_err::<Error, _>(|err| err.into()).flatten() {
							Ok(request) => {
								tokio::time::sleep(delay).await;
								connect_async(request).await.map_err(|err| err.into())
							},
							Err(err) => Err(err),
						}
					}
					.into_actor(this)
				})
				.map(|res, this, ctx| match res {
					Ok((stream, _)) => {
						debug!("connected");

						this
							.backend
							.upgrade()
							.map(|backend| backend.do_send(ConnectionState::Connected));

						let (tx, rx) = stream.split();
						ctx.add_stream(rx);
						this.sink = Some(SinkWrite::new(tx, ctx));
					},
					Err(err) => {
						warn!("connection failed: {err}");

						this.issue_system_async(DispatchUserMessage {
							aerodrome: Some(this.icao),
							message: format!("Could not connect to network: {err}"),
						});
						this.fail(ctx);
					},
				}),
		);
	}

	fn stopped(&mut self, _ctx: &mut Self::Context) {
		debug!("stopped: {}", self.icao);

		self.sink.as_mut().map(|sink| sink.close());
	}
}

impl Handler<BackendUpstream> for Network {
	type Result = ();

	fn handle(
		&mut self,
		BackendUpstream(message): BackendUpstream,
		_ctx: &mut Self::Context,
	) {
		match &message {
			Upstream::SharedStateUpdate { .. } => {
				trace!("{} <- SharedStateUpdate", self.icao);
			},
			Upstream::MultiStateUpdate { .. } => {
				trace!("{} <- MultiStateUpdate", self.icao);
			},
			_ => trace!("{} <- {message:?}", self.icao),
		}

		match serde_json::to_string(&message) {
			Ok(message) => {
				let message = tungstenite::Message::Text(message.into());
				let _ = self.sink.as_mut().map(|sink| sink.write(message));
			},
			Err(err) => warn!("json serialisation error: {err}"),
		}
	}
}

impl StreamHandler<Result<tungstenite::Message, tungstenite::Error>>
	for Network
{
	fn handle(
		&mut self,
		res: Result<tungstenite::Message, tungstenite::Error>,
		ctx: &mut Self::Context,
	) {
		match res {
			Ok(message) => match message {
				tungstenite::Message::Text(message) => {
					match serde_json::from_str::<Downstream<Option<Patch>>>(&message) {
						Ok(message) => {
							trace!("{} -> {message:?}", self.icao);
							self
								.backend
								.upgrade()
								.map(|backend| backend.do_send(BackendDownstream(message)));
						},
						Err(err) => warn!("json deserialisation error: {err}"),
					}
				},
				tungstenite::Message::Close(frame) => {
					debug!("socket close: {frame:?}");
					self.fail(ctx);
				},
				_ => (),
			},
			Err(err) => {
				warn!("socket read error: {err}");
				self.fail(ctx);
			},
		}
	}
}

impl WriteHandler<tungstenite::Error> for Network {
	fn error(
		&mut self,
		err: tungstenite::Error,
		ctx: &mut Self::Context,
	) -> Running {
		warn!("socket write error: {err}");
		self.fail(ctx);
		Running::Stop
	}
}
