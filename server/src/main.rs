mod aerodrome;
mod api;
mod client;
mod config;
mod connection;
mod server;
mod settings;
mod update;

use crate::client::{Client, ClientInit};
use crate::server::{AddClient, ClientId, Server};
use crate::update::Update;

use std::net::Ipv4Addr;
use std::time::Duration;

use actix::{Addr, Arbiter};
use anyhow::Result;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

#[actix::main]
async fn main() -> Result<()> {
	bars_tracing::init!()?;

	let update = tokio::spawn(Update::begin());

	let arbiter = Arbiter::new();

	let socket = bind().await?;

	let server = Server::new();
	let mut id: ClientId = 0;

	loop {
		const TIMEOUT: Duration = Duration::from_secs(1);

		tokio::select! {
			res = socket.accept() => match res {
				Ok((stream, remote)) => {
					debug!("accepted from {remote}");

					let server = server.clone();
					id += 1;

					tokio::spawn(async move {
						if let Err(err) = handle(stream, id, server).await {
							warn!("error handling {remote}: {err}");
						}
					});
				},
				Err(err) => {
					warn!("failed to accept: {err}");
					tokio::time::sleep(TIMEOUT).await;
				},
			},
			_ = tokio::time::sleep(Duration::from_secs(5)) => {
				if !server.connected() {
					break
				}
			},
		}
	}

	drop(arbiter);

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

async fn handle(
	mut stream: TcpStream,
	id: ClientId,
	server: Addr<Server>,
) -> Result<()> {
	let init_byte = stream.read_u8().await?;
	if init_byte == bars_ipc::TCP_INIT_BYTE {
		server.do_send(AddClient {
			id,
			addr: Client::new_tcp(
				stream,
				ClientInit {
					id,
					server: server.clone(),
				},
			),
		});
	} else {
		warn!("unhandled client with nonconformant initial byte {init_byte:02x}");
	}

	Ok(())
}
