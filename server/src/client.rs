use crate::server::{
	ClientId, IpcDownstream, IpcUpstream, RemoveClient, Server,
};

use std::collections::HashSet;
use std::io;

use bars_ipc::tcp::Channel;
use bars_ipc::{Codec, Downstream, Upstream, tcp};

use actix::io::{FramedWrite, WriteHandler};
use actix::{
	Actor, ActorContext, Addr, Arbiter, AsyncContext, Context, Handler, Message,
	StreamHandler,
};
use actix_broker::BrokerSubscribe;
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tracing::{instrument, trace, warn};

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct UserMessage {
	pub aerodrome: Option<String>,
	pub message: String,
}

pub struct Client {
	init: ClientInit,
	sink: ClientSink,
	subscriptions: HashSet<String>,
}

#[derive(Debug)]
pub struct ClientInit {
	pub id: ClientId,
	pub server: Addr<Server>,
}

impl Client {
	#[instrument(level = "debug")]
	pub fn new_tcp(stream: TcpStream, init: ClientInit) -> Addr<Self> {
		let (rx, tx) = Channel::accept(stream).into_split();

		Self::start_in_arbiter(&Arbiter::current(), move |ctx| {
			ctx.add_stream(rx.into_stream());
			Self::new_inner(ClientSink::new_tcp(tx, ctx), init)
		})
	}

	fn new_inner(sink: ClientSink, init: ClientInit) -> Self {
		Self {
			init,
			sink,
			subscriptions: HashSet::new(),
		}
	}
}

impl Actor for Client {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		self.subscribe_system_async::<UserMessage>(ctx);
	}

	fn stopped(&mut self, _ctx: &mut Self::Context) {
		for aerodrome in std::mem::take(&mut self.subscriptions) {
			self.init.server.do_send(IpcUpstream {
				client: self.init.id,
				message: Upstream::Subscribe {
					aerodrome,
					subscribe: false,
				},
			});
		}

		self.init.server.do_send(RemoveClient { id: self.init.id });
	}
}

impl Handler<UserMessage> for Client {
	type Result = ();

	fn handle(&mut self, message: UserMessage, _ctx: &mut Self::Context) {
		if message
			.aerodrome
			.is_none_or(|aerodrome| self.subscriptions.contains(&aerodrome))
		{
			self.sink.send(Downstream::UserMessage(message.message));
		}
	}
}

impl Handler<IpcDownstream> for Client {
	type Result = ();

	fn handle(&mut self, message: IpcDownstream, _ctx: &mut Self::Context) {
		if !matches!(
			message,
			IpcDownstream {
				message: Downstream::OpenAerodrome { .. }
					| Downstream::MapUpdate { .. }
			}
		) {
			trace!("{} <- {:?}", self.init.id, message.message);
		}

		self.sink.send(message.message);
	}
}

impl StreamHandler<io::Result<Upstream>> for Client {
	fn handle(&mut self, item: io::Result<Upstream>, ctx: &mut Self::Context) {
		match item {
			Ok(message) => {
				trace!("{} -> {message:?}", self.init.id);

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

				self.init.server.do_send(IpcUpstream {
					client: self.init.id,
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
