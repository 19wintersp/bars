mod actor;
mod route;
mod settings;
mod update;

use crate::actor::service::core::CoreService;
use crate::route::Router;
use crate::update::Update;

use std::net::Ipv4Addr;
use std::time::Duration;

use actix::{Arbiter, System, SystemService};
use anyhow::Result;
use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	bars_tracing::init!()?;

	let socket = bind().await?;
	let update = tokio::spawn(Update::begin());
	let router = Router::new();

	tokio::task::spawn_blocking(move || {
		let system = System::new();

		CoreService::from_registry();

		system.runtime().spawn(async move {
			loop {
				const TIMEOUT: Duration = Duration::from_secs(1);

				match socket.accept().await {
					Ok((stream, remote)) => {
						debug!("accepted from {remote}");

						let router = router.clone();
						Arbiter::current().spawn(async move {
							if let Err(err) = router.handle(stream).await {
								warn!("error handling {remote}: {err}");
							} else {
								debug!("closed {remote}");
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
