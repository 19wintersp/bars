use crate::actor::service::core::{CoreService, RemoveClient};

use std::collections::HashSet;
use std::io;

use bars_config::Icao;
use bars_ipc::tcp::Channel;
use bars_ipc::{Codec, Downstream, Upstream, tcp};

use actix::io::{FramedWrite, WriteHandler};
use actix::{
	Actor, ActorContext, Addr, AsyncContext, Context, Handler, Message,
	StreamHandler, SystemService,
};
use actix_broker::BrokerSubscribe;
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tracing::{trace, warn};

pub type ClientId = u64;

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientDownstream {
	pub message: Downstream,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientUpstream {
	pub client: ClientId,
	pub message: Upstream,
}

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct DispatchUserMessage {
	pub aerodrome: Option<Icao>,
	pub message: String,
}

pub struct Client {
	id: ClientId,
	sink: ClientSink,
	subscriptions: HashSet<Icao>,
}

impl Client {
	pub fn new_tcp(stream: TcpStream, id: ClientId) -> Addr<Self> {
		let (rx, tx) = Channel::accept(stream).into_split();

		Self::create(move |ctx| {
			ctx.add_stream(rx.into_stream());
			Self {
				id,
				sink: ClientSink::new_tcp(tx, ctx),
				subscriptions: HashSet::new(),
			}
		})
	}
}

impl Actor for Client {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		self.subscribe_system_async::<DispatchUserMessage>(ctx);
	}

	fn stopped(&mut self, _ctx: &mut Self::Context) {
		let core = CoreService::from_registry();

		for aerodrome in std::mem::take(&mut self.subscriptions) {
			core.do_send(ClientUpstream {
				client: self.id,
				message: Upstream::Subscribe {
					aerodrome,
					subscribe: false,
				},
			});
		}

		core.do_send(RemoveClient { id: self.id });
	}
}

impl Handler<DispatchUserMessage> for Client {
	type Result = ();

	fn handle(&mut self, message: DispatchUserMessage, _ctx: &mut Self::Context) {
		if message
			.aerodrome
			.is_none_or(|aerodrome| self.subscriptions.contains(&aerodrome))
		{
			self.sink.send(Downstream::UserMessage(message.message));
		}
	}
}

impl Handler<ClientDownstream> for Client {
	type Result = ();

	fn handle(&mut self, message: ClientDownstream, _ctx: &mut Self::Context) {
		match &message.message {
			Downstream::Aerodrome {
				aerodrome,
				config,
				state,
				updates,
			} => trace!(
				"{} <- Aerodrome {{ \
					aerodrome: {aerodrome:?}, config: {}, state: {state:?}, updates: {} \
				}}",
				self.id,
				if config.is_some() { "Some" } else { "None" },
				updates.len(),
			),
			other => trace!("{} <- {other:?}", self.id),
		}

		self.sink.send(message.message);
	}
}

impl StreamHandler<io::Result<Upstream>> for Client {
	fn handle(&mut self, item: io::Result<Upstream>, ctx: &mut Self::Context) {
		match item {
			Ok(message) => {
				trace!("{} -> {message:?}", self.id);

				if let Upstream::Subscribe {
					aerodrome,
					subscribe,
				} = &message
				{
					let valid = if *subscribe {
						self.subscriptions.insert(aerodrome.clone())
					} else {
						self.subscriptions.remove(aerodrome)
					};

					if !valid {
						return
					}
				}

				CoreService::from_registry().do_send(ClientUpstream {
					client: self.id,
					message,
				});
			},
			Err(err) => {
				warn!("client read error: {err}");
				ctx.stop();
			},
		}
	}
}

impl WriteHandler<io::Error> for Client {}

enum ClientSink {
	Tcp(FramedWrite<Downstream, OwnedWriteHalf, Codec<Downstream>>),
}

impl ClientSink {
	pub fn new_tcp(
		tx: tcp::Sender<Downstream>,
		ctx: &mut Context<Client>,
	) -> Self {
		Self::Tcp(FramedWrite::new(tx.into_inner(), Codec::new(), ctx))
	}

	pub fn send(&mut self, message: Downstream) {
		match self {
			Self::Tcp(write) => write.write(message),
		}
	}
}
