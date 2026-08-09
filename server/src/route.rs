mod api;
mod web;

use crate::actor::client::Client;

use anyhow::Result;
use axum::extract::Request;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tower::Service;

#[derive(Clone)]
pub struct Router {
	router: axum::Router,
}

impl Router {
	pub fn new() -> Self {
		Self {
			router: web::router().nest("/api", api::router()),
		}
	}

	pub async fn handle(self, mut stream: TcpStream) -> Result<()> {
		let init_byte = stream.read_u8().await?;
		if init_byte == bars_ipc::TCP_INIT_BYTE {
			Client::new_tcp(stream);
		} else {
			let (read, write) = stream.into_split();
			let read = tokio::io::repeat(init_byte).take(1).chain(read);
			let stream = TokioIo::new(tokio::io::join(read, write));

			let service = service_fn(move |request: Request<Incoming>| {
				self.router.clone().call(request)
			});

			hyper::server::conn::http1::Builder::new()
				.serve_connection(stream, service)
				.with_upgrades()
				.await?;
		}

		Ok(())
	}
}
