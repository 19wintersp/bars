use crate::{Downstream, Upstream};

use std::io::{Error, ErrorKind, Result};
use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf, ReuniteError};
use tokio::net::{TcpStream, ToSocketAddrs};

const MAX_BUF_SIZE: usize = 0x10_0000;

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
		self.rx.rx.reunite(self.tx.tx)
	}
}

impl<Rx: DeserializeOwned, Tx> Channel<Rx, Tx> {
	/// This method is **not** cancel-safe.
	pub async fn recv(&mut self) -> Result<Rx> {
		self.rx.recv().await
	}
}

impl<Rx, Tx: Serialize> Channel<Rx, Tx> {
	/// This method is **not** cancel-safe.
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

pub struct Receiver<Rx> {
	phantom: PhantomData<Rx>,
	rx: OwnedReadHalf,
}

impl<Rx> Receiver<Rx> {
	fn new(rx: OwnedReadHalf) -> Self {
		Self {
			phantom: PhantomData,
			rx,
		}
	}
}

impl<Rx: DeserializeOwned> Receiver<Rx> {
	/// This method is **not** cancel-safe.
	pub async fn recv(&mut self) -> Result<Rx> {
		let len = self.rx.read_u32().await? as usize;
		if len > MAX_BUF_SIZE {
			return Err(ErrorKind::FileTooLarge.into())
		}

		let mut buf = vec![0u8; len];
		self.rx.read_exact(&mut buf).await?;

		postcard::from_bytes(&buf)
			.map_err(|err| Error::new(ErrorKind::InvalidData, err))
	}
}

/// Note that dropping this type will shut down the TCP connection.
pub struct Sender<Tx> {
	phantom: PhantomData<Tx>,
	tx: OwnedWriteHalf,
}

impl<Tx> Sender<Tx> {
	fn new(tx: OwnedWriteHalf) -> Self {
		Self {
			phantom: PhantomData,
			tx,
		}
	}
}

impl<Tx: Serialize> Sender<Tx> {
	/// This method is **not** cancel-safe.
	pub async fn send(&mut self, frame: &Tx) -> Result<()> {
		let buf = postcard::to_stdvec(frame)
			.map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;

		self.tx.write_u32(buf.len() as u32).await?;
		self.tx.write_all(&buf).await?;

		Ok(())
	}
}
