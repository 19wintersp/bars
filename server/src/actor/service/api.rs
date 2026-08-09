mod base;

use self::base::Base;
use crate::actor::service::settings::{
	GetSettings, SetApiToken, SettingsService,
};
use crate::settings::ApiSettings;

use std::time::Duration;

use bars_config::Icao;

use actix::{
	Actor, ActorFutureExt, AsyncContext, Context, Handler, Message,
	ResponseFuture, Supervised, SystemService, WrapFuture,
};
use anyhow::{Result, bail};
use async_tungstenite::tungstenite::client::IntoClientRequest;
use bytes::Bytes;
use http::request::Builder as RequestBuilder;
use http::{Request, header};
use reqwest::{Client, Response};
use serde::Deserialize;
use tracing::{error, instrument, warn};

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
	pub aerodrome: Icao,
}

#[derive(Message)]
#[rtype(result = "Result<bool>")]
pub struct FetchIsOnline;

#[derive(Message)]
#[rtype(result = "Result<Bytes>")]
pub struct FetchConfig {
	pub aerodrome: Icao,
}

pub struct ApiService {
	api: Base,
	cdn: Base,
	token: Option<String>,
	client: Client,
}

impl ApiService {
	fn load_settings(&mut self, settings: ApiSettings) {
		fn base(from: &Option<String>, name: &str) -> Option<Base> {
			from.as_ref().and_then(|base| {
				Base::new(base).or_else(|| {
					warn!("invalid {name} base uri specified");
					None
				})
			})
		}

		self.api = base(&settings.api, "api").unwrap_or_else(Base::default_api);
		self.cdn = base(&settings.cdn, "cdn").unwrap_or_else(Base::default_cdn);
		self.token = settings.token;
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

impl Default for ApiService {
	fn default() -> Self {
		Self {
			api: Base::default_api(),
			cdn: Base::default_cdn(),
			token: None,
			client: Client::new(),
		}
	}
}

impl Actor for ApiService {
	type Context = Context<Self>;

	fn started(&mut self, ctx: &mut Self::Context) {
		ctx.wait(
			SettingsService::from_registry()
				.send(GetSettings)
				.into_actor(self)
				.map(|res, this, _ctx| match res {
					Ok(settings) => this.load_settings(settings.api),
					Err(err) => error!("failed to get settings: {err}"),
				}),
		);
	}
}

impl Supervised for ApiService {
	fn restarting(&mut self, ctx: &mut <Self as Actor>::Context) {
		self.started(ctx);
	}
}

impl SystemService for ApiService {}

impl Handler<CreateConnectRequest> for ApiService {
	type Result = Result<Request<()>>;

	#[instrument(level = "trace", skip(self, _ctx))]
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

impl Handler<FetchIsOnline> for ApiService {
	type Result = ResponseFuture<Result<bool>>;

	#[instrument(level = "trace", skip_all)]
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

impl Handler<FetchConfig> for ApiService {
	type Result = ResponseFuture<Result<Bytes>>;

	#[instrument(level = "trace", skip(self, _ctx))]
	fn handle(
		&mut self,
		FetchConfig { aerodrome }: FetchConfig,
		_ctx: &mut Self::Context,
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

impl Handler<SetApiToken> for ApiService {
	type Result = ();

	fn handle(
		&mut self,
		SetApiToken(token): SetApiToken,
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
