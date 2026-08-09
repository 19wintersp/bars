use crate::actor::service::core::{AddClient, CoreService, RemoveClient};

use std::collections::HashSet;
use std::io;

use bars_config::Icao;
use bars_ipc::tcp::Channel;
use bars_ipc::{Codec, Downstream, Upstream, tcp};

use actix::io::{FramedWrite, SinkWrite, WriteHandler};
use actix::{
	Actor, ActorContext, AsyncContext, Context, Handler, Message, StreamHandler,
	SystemService,
};
use actix_broker::BrokerSubscribe;
use async_tungstenite::tokio::TokioAdapter;
use async_tungstenite::{WebSocketSender, WebSocketStream, tungstenite};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tracing::{debug, trace, warn};

type WsIo = TokioAdapter<TokioIo<Upgraded>>;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClientId(u32);

impl ClientId {
	fn next() -> Self {
		use std::sync::atomic::{AtomicU32, Ordering};

		static CURRENT: AtomicU32 = AtomicU32::new(1);

		Self(CURRENT.fetch_add(1, Ordering::SeqCst))
	}
}

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

pub enum Transport {
	Json,
	Postcard,
}

pub struct Client {
	id: ClientId,
	sink: ClientSink,
	subscriptions: HashSet<Icao>,
}

impl Client {
	pub fn new_tcp(stream: TcpStream) {
		let (rx, tx) = Channel::accept(stream).into_split();
		Self::new(move |ctx| {
			ctx.add_stream(rx.into_stream());
			ClientSink::new_tcp(tx, ctx)
		});
	}

	pub fn new_ws(stream: WebSocketStream<WsIo>, transport: Transport) {
		let (tx, rx) = stream.split();
		Self::new(move |ctx| {
			ctx.add_stream(rx);
			ClientSink::new_ws(tx, transport, ctx)
		});
	}

	fn new(f: impl FnOnce(&mut Context<Self>) -> ClientSink) {
		let id = ClientId::next();

		CoreService::from_registry().do_send(AddClient {
			id,
			addr: Self::create(move |ctx| Self {
				id,
				sink: f(ctx),
				subscriptions: HashSet::new(),
			}),
		});
	}

	fn handle_upstream(&mut self, message: Upstream) {
		trace!("{} -> {message:?}", self.id.0);

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
			self.sink.send(Downstream::UserMessage {
				message: message.message,
			});
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
				self.id.0,
				if config.is_some() { "Some" } else { "None" },
				updates.len(),
			),
			other => trace!("{} <- {other:?}", self.id.0),
		}

		self.sink.send(message.message);
	}
}

impl StreamHandler<io::Result<Upstream>> for Client {
	fn handle(&mut self, item: io::Result<Upstream>, ctx: &mut Self::Context) {
		match item {
			Ok(message) => {
				self.handle_upstream(message);
			},
			Err(err) => {
				warn!("client read error: {err}");
				ctx.stop();
			},
		}
	}
}

impl StreamHandler<tungstenite::Result<tungstenite::Message>> for Client {
	fn handle(
		&mut self,
		item: tungstenite::Result<tungstenite::Message>,
		ctx: &mut Self::Context,
	) {
		use tungstenite::Message;

		match item {
			Ok(Message::Text(text)) => match serde_json::from_str(&text) {
				Ok(message) => self.handle_upstream(message),
				Err(err) => warn!("json deserialisation error: {err}"),
			},
			Ok(Message::Binary(bytes)) => match postcard::from_bytes(&bytes) {
				Ok(message) => self.handle_upstream(message),
				Err(err) => warn!("postcard deserialisation error: {err}"),
			},
			Ok(Message::Close(frame)) => {
				debug!("client closed: {frame:?}");
				ctx.stop();
			},
			Ok(_) => (),
			Err(err) => {
				warn!("client (ws) read error: {err}");
				ctx.stop();
			},
		}
	}
}

impl WriteHandler<io::Error> for Client {}

impl WriteHandler<tungstenite::Error> for Client {}

enum ClientSink {
	Tcp(FramedWrite<Downstream, OwnedWriteHalf, Codec<Downstream>>),
	Ws(
		SinkWrite<tungstenite::Message, WebSocketSender<WsIo>>,
		Transport,
	),
}

impl ClientSink {
	fn new_tcp(tx: tcp::Sender<Downstream>, ctx: &mut Context<Client>) -> Self {
		Self::Tcp(FramedWrite::new(tx.into_inner(), Codec::new(), ctx))
	}

	fn new_ws(
		tx: WebSocketSender<WsIo>,
		transport: Transport,
		ctx: &mut Context<Client>,
	) -> Self {
		Self::Ws(SinkWrite::new(tx, ctx), transport)
	}

	fn send(&mut self, message: Downstream) {
		match self {
			Self::Tcp(writer) => writer.write(message),
			Self::Ws(writer, Transport::Json) => {
				match serde_json::to_string(&message) {
					Ok(text) => {
						let _ = writer.write(tungstenite::Message::Text(text.into()));
					},
					Err(err) => warn!("json serialisation error: {err}"),
				}
			},
			Self::Ws(writer, Transport::Postcard) => {
				match postcard::to_stdvec(&message) {
					Ok(bytes) => {
						let _ = writer.write(tungstenite::Message::Binary(bytes.into()));
					},
					Err(err) => warn!("postcard serialisation error: {err}"),
				}
			},
		}
	}
}
