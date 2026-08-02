mod base;

use self::base::Base;
use crate::settings::ApiSettings;

use std::time::Duration;

use actix::{Actor, Addr, Arbiter, Context, Handler, Message, ResponseFuture};
use anyhow::{Result, bail};
use async_tungstenite::tungstenite::client::IntoClientRequest;
use bytes::Bytes;
use http::request::Builder as RequestBuilder;
use http::{Request, header};
use reqwest::{Client, Response};
use serde::Deserialize;
use tracing::warn;

static USER_AGENT: &str = concat!(
	env!("CARGO_PKG_NAME"),
	"/",
	env!("CARGO_PKG_VERSION"),
	" (EuroScope)"
);

const IS_ONLINE_DELAY: Duration = Duration::from_secs(2);

#[derive(Message)]
#[rtype(result = "Result<Request<()>>")]
pub struct CreateConnectRequest {
	pub aerodrome: String,
}

#[derive(Message)]
#[rtype(result = "Result<bool>")]
pub struct FetchIsOnline;

#[derive(Message)]
#[rtype(result = "Result<Bytes>")]
pub struct FetchConfig {
	pub aerodrome: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct UpdateToken(pub Option<String>);

pub struct ApiManager {
	api: Base,
	cdn: Base,
	token: Option<String>,
	client: Client,
}

impl ApiManager {
	pub fn new(settings: &ApiSettings) -> Addr<Self> {
		fn base(from: &Option<String>, name: &str) -> Option<Base> {
			from.as_ref().and_then(|base| {
				Base::new(base).or_else(|| {
					warn!("invalid {name} base uri specified");
					None
				})
			})
		}

		let this = Self {
			api: base(&settings.api, "api").unwrap_or_else(Base::default_api),
			cdn: base(&settings.cdn, "cdn").unwrap_or_else(Base::default_cdn),
			token: settings.token.clone(),
			client: Client::new(),
		};

		Self::start_in_arbiter(&Arbiter::current(), |_ctx| this)
	}

	fn create_request(&self) -> Result<RequestBuilder> {
		if let Some(token) = self.token.as_ref() {
			Ok(
				RequestBuilder::new()
					.header(header::AUTHORIZATION, format!("Bearer {token}"))
					.header(header::USER_AGENT, USER_AGENT),
			)
		} else {
			bail!("not authenticated");
		}
	}

	fn execute_request(
		&self,
		request: RequestBuilder,
	) -> Result<impl Future<Output = Result<Response, reqwest::Error>> + 'static>
	{
		let mut request: reqwest::Request = request.body("")?.try_into()?;
		request.body_mut().take();
		Ok(self.client.execute(request))
	}
}

impl Actor for ApiManager {
	type Context = Context<Self>;
}

impl Handler<CreateConnectRequest> for ApiManager {
	type Result = Result<Request<()>>;

	fn handle(
		&mut self,
		CreateConnectRequest { aerodrome }: CreateConnectRequest,
		_ctx: &mut Self::Context,
	) -> Self::Result {
		let uri = self
			.api
			.create_ws_uri(&format!("/connect?airport={aerodrome}"))?;
		to_ws_request(self.create_request()?.uri(uri).body(())?)
	}
}

impl Handler<FetchIsOnline> for ApiManager {
	type Result = ResponseFuture<Result<bool>>;

	fn handle(
		&mut self,
		_: FetchIsOnline,
		_: &mut Self::Context,
	) -> Self::Result {
		#[derive(Deserialize)]
		struct NetworkStatus {
			offline: Option<bool>,
		}

		let request = self
			.api
			.create_http_uri("/auth/network-status")
			.and_then(|uri| self.execute_request(self.create_request()?.uri(uri)));

		Box::pin(async move {
			tokio::time::sleep(IS_ONLINE_DELAY).await;

			Ok(
				request?
					.await?
					.error_for_status()?
					.json::<NetworkStatus>()
					.await?
					.offline
					.is_some_and(|offline| !offline),
			)
		})
	}
}

impl Handler<FetchConfig> for ApiManager {
	type Result = ResponseFuture<Result<Bytes>>;

	fn handle(
		&mut self,
		FetchConfig { aerodrome }: FetchConfig,
		_: &mut Self::Context,
	) -> Self::Result {
		let request = self
			.cdn
			.create_http_uri(&format!("/EuroScope/{aerodrome}/config"))
			.and_then(|uri| self.execute_request(self.create_request()?.uri(uri)));

		Box::pin(
			async move { Ok(request?.await?.error_for_status()?.bytes().await?) },
		)
	}
}

impl Handler<UpdateToken> for ApiManager {
	type Result = ();

	fn handle(
		&mut self,
		UpdateToken(token): UpdateToken,
		_ctx: &mut Self::Context,
	) {
		self.token = token;
	}
}

fn to_ws_request(mut request: Request<()>) -> Result<Request<()>> {
	let mut out = request.uri().into_client_request()?;
	let headers = std::mem::take(request.headers_mut());
	out.headers_mut().extend(headers);
	Ok(out)
}
