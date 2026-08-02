use std::net::Ipv4Addr;
use std::time::Duration;

use bars_ipc::tcp::Channel;
use bars_ipc::{Downstream, Upstream};

use anyhow::Result;
use tokio::sync::mpsc::{
	UnboundedReceiver, UnboundedSender, unbounded_channel,
};
use tracing::{debug, error, instrument, warn};

pub struct Client {
	rx: UnboundedReceiver<Downstream>,
	tx: UnboundedSender<Upstream>,
}

impl Client {
	pub fn new() -> Self {
		let (dn_tx, dn_rx) = unbounded_channel();
		let (up_tx, up_rx) = unbounded_channel();

		if let Err(err) = spawn_worker(up_rx, dn_tx) {
			error!("failed to start client worker: {err}");
		}

		Self {
			rx: dn_rx,
			tx: up_tx,
		}
	}

	pub fn connect(&mut self) {
		*self = Self::new();
	}

	pub fn send(&self, message: Upstream) {
		let _ = self.tx.send(message);
	}

	pub fn queue(&mut self) -> impl Iterator<Item = Downstream> {
		std::iter::from_fn(|| self.rx.try_recv().ok())
	}

	pub fn is_connected(&self) -> bool {
		!self.tx.is_closed()
	}
}

#[instrument(skip_all)]
fn spawn_worker(
	rx: UnboundedReceiver<Upstream>,
	tx: UnboundedSender<Downstream>,
) -> Result<()> {
	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()?;

	std::thread::Builder::new()
		.name("client".into())
		.spawn(move || {
			runtime.block_on(async {
				debug!("client worker started");

				if let Err(err) = worker(rx, tx).await {
					error!("worker error: {err}");
				} else {
					debug!("client worker shutting down");
				}
			});

			debug!("client worker shut down");
		})?;

	Ok(())
}

async fn worker(
	rx: UnboundedReceiver<Upstream>,
	tx: UnboundedSender<Downstream>,
) -> Result<()> {
	use bars_ipc::tcp::{Receiver, Sender};

	async fn up(
		mut rx: UnboundedReceiver<Upstream>,
		mut tx: Sender<Upstream>,
	) -> Result<()> {
		loop {
			match rx.recv().await {
				Some(message) => tx.send(&message).await?,
				None => break Ok(()),
			}
		}
	}

	async fn dn(
		tx: UnboundedSender<Downstream>,
		mut rx: Receiver<Downstream>,
	) -> Result<()> {
		loop {
			if tx.send(rx.recv().await?).is_err() {
				break Ok(())
			}
		}
	}

	tokio::time::sleep(Duration::from_secs(1)).await;

	let (channel_rx, channel_tx) =
		Channel::connect((Ipv4Addr::LOCALHOST, bars_ipc::PORT))
			.await?
			.into_split();

	debug!("worker loop starting");

	tokio::select! {
		res = up(rx, channel_tx) => {
			match res {
				Ok(()) => debug!("worker close due context (tx) shutdown"),
				Err(err) => warn!("worker close due server tx error: {err}"),
			}
		},
		res = dn(tx, channel_rx) => {
			match res {
				Ok(()) => debug!("worker close due context (rx) shutdown"),
				Err(err) => warn!("worker close due server rx error: {err}"),
			}
		},
	};

	Ok(())
}
