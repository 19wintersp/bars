#[cfg(feature = "codec")]
mod codec;
#[cfg(feature = "tcp")]
pub mod tcp;

#[cfg(feature = "codec")]
pub use self::codec::Codec;

use bars_config::{
	Aerodrome, Block, BlockState, GeoMap, Map, Node, Preset, Profile, Ref, State,
	Style,
};
use bars_graph::MapUpdate;

use serde::{Deserialize, Serialize};

pub const PORT: u16 = 21314;
pub const TCP_INIT_BYTE: u8 = 0xba;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Upstream {
	Authenticate {
		token: Option<String>,
	},
	Connect {
		capacity: ConnectionType,
	},
	Subscribe {
		aerodrome: String,
		subscribe: bool,
	},
	GraphAction {
		aerodrome: String,
		action: GraphAction,
	},
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum GraphAction {
	SetProfile(Ref<Profile>),
	ApplyPreset(Ref<Preset>),
	SetNodeState(Ref<Node>, State),
	SetBlockState(Ref<Block>, BlockState),
	InsertRoute(Ref<Node>, Ref<Node>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Downstream {
	Hello,
	UserMessage(String),
	Connection {
		capacity: ConnectionType,
	},
	OpenAerodrome {
		aerodrome: String,
		config: AerodromeConfig,
	},
	CloseAerodrome {
		aerodrome: String,
	},
	MapUpdate {
		aerodrome: String,
		update: MapUpdate,
	},
	Pilots {
		callsigns: Vec<String>,
	},
}

#[derive(
	Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize,
)]
pub enum ConnectionType {
	None,
	Observe,
	Control,
	//Local,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AerodromeConfig {
	pub config: Aerodrome,
	pub geo_map: Option<GeoMap>,
	pub maps: Vec<Map>,
	pub styles: Vec<Style>,
}
