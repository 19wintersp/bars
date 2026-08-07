use crate::actor::service::api::ApiService;
use crate::settings::Settings;

use actix::{Actor, Context, Handler, Message, MessageResult, Supervised, SystemService};
use tracing::error;

#[derive(Message)]
#[rtype(result = "Settings")]
pub struct GetSettings;

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetApiToken(pub Option<String>);

pub struct SettingsService {
	settings: Settings,
}

impl SettingsService {
	fn new() -> Self {
		Self {
			settings: Settings::load()
				.inspect_err(|err| error!("failed to load settings: {err}"))
				.unwrap_or_default(),
		}
	}
}

impl Default for SettingsService {
	fn default() -> Self {
		Self::new()
	}
}

impl Actor for SettingsService {
	type Context = Context<Self>;
}

impl Supervised for SettingsService {}

impl SystemService for SettingsService {}

impl Handler<GetSettings> for SettingsService {
	type Result = MessageResult<GetSettings>;

	fn handle(&mut self, _: GetSettings, _ctx: &mut Self::Context) -> Self::Result {
		MessageResult(self.settings.clone())
	}
}

impl Handler<SetApiToken> for SettingsService {
	type Result = ();

	fn handle(
		&mut self,
		SetApiToken(token): SetApiToken,
		_ctx: &mut Self::Context,
	) {
		self.settings.api.token = token.clone();
		if let Err(err) = self.settings.save() {
			error!("failed to save settings: {err}");
		}

		ApiService::from_registry().do_send(SetApiToken(token));
	}
}
