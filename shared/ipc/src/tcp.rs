use crate::{Codec, Downstream, Upstream};

use std::io::{ErrorKind, Result};

use futures::{Sink, Stream};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf, ReuniteError};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::{FramedRead, FramedWrite};

pub struct Channel<Rx, Tx> {
	rx: Receiver<Rx>,
	tx: Sender<Tx>,
}

impl<Rx, Tx> Channel<Rx, Tx> {
	fn new(stream: TcpStream) -> Self {
		let (rx, tx) = stream.into_split();
		Self {
			rx: Receiver::new(rx),
			tx: Sender::new(tx),
		}
	}

	pub fn into_split(self) -> (Receiver<Rx>, Sender<Tx>) {
		(self.rx, self.tx)
	}

	pub fn from_split(rx: Receiver<Rx>, tx: Sender<Tx>) -> Self {
		Self { rx, tx }
	}

	pub fn into_stream(self) -> std::result::Result<TcpStream, ReuniteError> {
		self.rx.into_inner().reunite(self.tx.into_inner())
	}
}

impl<Rx: DeserializeOwned, Tx> Channel<Rx, Tx> {
	pub async fn recv(&mut self) -> Result<Rx> {
		self.rx.recv().await
	}
}

impl<Rx, Tx: Serialize> Channel<Rx, Tx> {
	pub async fn send(&mut self, frame: &Tx) -> Result<()> {
		self.tx.send(frame).await
	}
}

impl Channel<Upstream, Downstream> {
	/// This method assumes the initialisation byte has been read already.
	pub fn accept(stream: TcpStream) -> Self {
		Self::new(stream)
	}
}

impl Channel<Downstream, Upstream> {
	pub async fn connect(addr: impl ToSocketAddrs) -> Result<Self> {
		let mut stream = TcpStream::connect(addr).await?;
		stream.write_u8(crate::TCP_INIT_BYTE).await?;
		Ok(Self::new(stream))
	}
}

pub struct Receiver<Rx>(FramedRead<OwnedReadHalf, Codec<Rx>>);

impl<Rx> Receiver<Rx> {
	fn new(rx: OwnedReadHalf) -> Self {
		Self(FramedRead::new(rx, Codec::new()))
	}

	pub fn into_inner(self) -> OwnedReadHalf {
		self.0.into_inner()
	}
}

impl<Rx: DeserializeOwned> Receiver<Rx> {
	pub async fn recv(&mut self) -> Result<Rx> {
		self
			.0
			.next()
			.await
			.ok_or(ErrorKind::UnexpectedEof.into())
			.flatten()
	}

	pub fn into_stream(self) -> impl Stream<Item = Result<Rx>> {
		self.0
	}
}

/// Note that dropping this type will shut down the TCP connection.
pub struct Sender<Tx>(FramedWrite<OwnedWriteHalf, Codec<Tx>>);

impl<Tx> Sender<Tx> {
	fn new(tx: OwnedWriteHalf) -> Self {
		Self(FramedWrite::new(tx, Codec::new()))
	}

	pub fn into_inner(self) -> OwnedWriteHalf {
		self.0.into_inner()
	}
}

impl<Tx: Serialize> Sender<Tx> {
	pub async fn send(&mut self, frame: &Tx) -> Result<()> {
		self.0.send(frame).await
	}

	pub fn sink_mut<'a>(&mut self) -> &mut impl Sink<&'a Tx> {
		&mut self.0
	}

	pub fn into_sink<'a>(self) -> impl Sink<&'a Tx> {
		self.0
	}
}
