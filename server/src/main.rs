mod actor;
mod settings;
mod update;

use crate::actor::client::{Client, ClientId};
use crate::actor::service::core::{AddClient, CoreService};
use crate::update::Update;

use std::net::Ipv4Addr;
use std::time::Duration;

use actix::{Arbiter, System, SystemService};
use anyhow::Result;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	bars_tracing::init!()?;

	let socket = bind().await?;
	let update = tokio::spawn(Update::begin());

	tokio::task::spawn_blocking(move || {
		let system = System::new();

		CoreService::from_registry();

		system.runtime().spawn(async move {
			let mut id: ClientId = 0;

			loop {
				const TIMEOUT: Duration = Duration::from_secs(1);

				match socket.accept().await {
					Ok((stream, remote)) => {
						debug!("accepted from {remote}");
						id += 1;
						Arbiter::current().spawn(async move {
							if let Err(err) = handle(stream, id).await {
								warn!("error handling {remote}: {err}");
							}
						});
					},
					Err(err) => {
						warn!("failed to accept: {err}");
						tokio::time::sleep(TIMEOUT).await;
					},
				}
			}
		});

		if let Err(err) = system.run() {
			error!("system: {err}");
		}
	})
	.await
	.inspect_err(|err| error!("system task: {err}"))?;

	info!("closing, committing update");

	if let Err(err) = update
		.await
		.map_err(|err| err.into())
		.flatten()
		.and_then(|update| update.commit())
	{
		error!("update failed: {err}");
	}

	Ok(())
}

async fn bind() -> Result<TcpListener> {
	const ATTEMPTS: usize = 3;
	const TIMEOUT: Duration = Duration::from_secs(5);

	for i in 1..=ATTEMPTS {
		debug!("binding server, attempt {i} of {ATTEMPTS}");

		match TcpListener::bind((Ipv4Addr::LOCALHOST, bars_ipc::PORT)).await {
			Ok(bound) => return Ok(bound.into()),
			Err(_) if i < ATTEMPTS => {
				warn!("failed to bind, retrying");
				tokio::time::sleep(TIMEOUT).await;
			},
			Err(err) => {
				error!("failed to bind: {err}");
				Err(err)?;
			},
		}
	}

	unreachable!();
}

async fn handle(mut stream: TcpStream, id: ClientId) -> Result<()> {
	let init_byte = stream.read_u8().await?;
	if init_byte == bars_ipc::TCP_INIT_BYTE {
		CoreService::from_registry().do_send(AddClient {
			id,
			addr: Client::new_tcp(stream, id),
		});
	} else {
		warn!("unhandled client with nonconformant initial byte {init_byte:02x}");
	}

	Ok(())
}
