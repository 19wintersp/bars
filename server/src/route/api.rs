use crate::actor::client::{Client, Transport};

use bars_ipc::{WS_PROTOCOL_JSON, WS_PROTOCOL_POSTCARD};

use actix::Arbiter;
use anyhow::Result;
use async_tungstenite::WebSocketStream;
use async_tungstenite::tokio::TokioAdapter;
use async_tungstenite::tungstenite::handshake::server::create_response;
use async_tungstenite::tungstenite::protocol::Role;
use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::routing::get;
use http::header::SEC_WEBSOCKET_PROTOCOL;
use http::{HeaderValue, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tracing::warn;

pub fn router() -> Router {
	Router::new().route("/connect", get(connect))
}

async fn connect(
	request: Request,
) -> Result<(Response<()>, Body), (StatusCode, &'static str)> {
	let request = request.map(|_| ());

	let Ok(mut response) = create_response(&request)
		.inspect_err(|err| warn!("failed to create ws response: {err}"))
	else {
		return Err((StatusCode::BAD_REQUEST, "websocket connection failed"))
	};

	let Some(transport) = request
		.headers()
		.get(SEC_WEBSOCKET_PROTOCOL)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| {
			for proto in value.split(',') {
				let proto = proto.trim();
				if proto == WS_PROTOCOL_JSON {
					return Some(Transport::Json)
				} else if proto == WS_PROTOCOL_POSTCARD {
					return Some(Transport::Postcard)
				}
			}
			None
		})
	else {
		return Err((StatusCode::BAD_REQUEST, "unsupported sub-protocol"))
	};

	response.headers_mut().insert(
		SEC_WEBSOCKET_PROTOCOL,
		HeaderValue::from_static(match transport {
			Transport::Json => WS_PROTOCOL_JSON,
			Transport::Postcard => WS_PROTOCOL_POSTCARD,
		}),
	);

	Arbiter::current().spawn(async move {
		match hyper::upgrade::on(request).await {
			Ok(upgraded) => {
				let stream = WebSocketStream::from_raw_socket(
					TokioAdapter::new(TokioIo::new(upgraded)),
					Role::Server,
					None,
				)
				.await;

				Client::new_ws(stream, transport);
			},
			Err(err) => warn!("failed to upgrade: {err}"),
		}
	});

	Ok((response, Body::empty()))
}
