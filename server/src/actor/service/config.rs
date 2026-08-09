use crate::actor::service::api::{ApiService, FetchConfig};

use std::collections::HashMap;
use std::sync::Arc;

use bars_config::{Config, Icao, Loadable};
use bars_ipc::AerodromeConfig;

use actix::{
	Actor, ActorFutureExt, Context, Handler, Message, ResponseActFuture,
	Supervised, SystemService, WrapFuture,
};
use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes};
use tracing::{info, warn};

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClearCache;

#[derive(Message)]
#[rtype(result = "Result<Arc<AerodromeConfig>>")]
pub struct GetConfig {
	pub aerodrome: Icao,
}

#[derive(Default)]
pub struct ConfigService {
	cache: HashMap<Icao, Arc<AerodromeConfig>>,
}

impl ConfigService {
	fn load(&mut self, icao: Icao, data: Bytes) -> Result<Arc<AerodromeConfig>> {
		let config = Config::load(data.reader())?;

		info!("loaded config {:?} ver {:?}", config.name, config.version);

		let aerodrome = config
			.aerodromes
			.into_iter()
			.find(|(key, _)| key == &icao)
			.ok_or(anyhow!("config does not contain advertised aerodrome"))?
			.1;
		let mut loaded = AerodromeConfig {
			config: aerodrome,
			geo_map: None,
			maps: Vec::new(),
			styles: Vec::new(),
		};

		for (key, mut maps) in config.maps {
			if key == icao {
				maps
					.rebase(&loaded.config, loaded.styles.len())
					.map_err(|id| anyhow!("invalid config: unknown id {id:?}"))?;

				if let Some(geo_map) = maps.geo_map {
					loaded.geo_map = Some(geo_map);
				}
				loaded.maps.extend(maps.maps);
				loaded.styles.extend(maps.styles);
			}
		}

		let config = Arc::new(loaded);
		self.cache.insert(icao, config.clone());
		Ok(config)
	}
}

impl Actor for ConfigService {
	type Context = Context<Self>;
}

impl Supervised for ConfigService {}

impl SystemService for ConfigService {}

impl Handler<ClearCache> for ConfigService {
	type Result = ();

	fn handle(&mut self, _: ClearCache, _: &mut Self::Context) {
		self.cache.clear();
	}
}

impl Handler<GetConfig> for ConfigService {
	type Result = ResponseActFuture<Self, Result<Arc<AerodromeConfig>>>;

	fn handle(
		&mut self,
		GetConfig { aerodrome }: GetConfig,
		_ctx: &mut Self::Context,
	) -> Self::Result {
		if let Some(config) = self.cache.get(&aerodrome).cloned() {
			Box::pin(actix::fut::ready(Ok(config)))
		} else {
			Box::pin(
				ApiService::from_registry()
					.send(FetchConfig { aerodrome })
					.into_actor(self)
					.map(move |res, this, _ctx| {
						res
							.map_err(|err| err.into())
							.flatten()
							.inspect_err(|err| warn!("failed to fetch config: {err}"))
							.and_then(|data| this.load(aerodrome, data))
					}),
			)
		}
	}
}
