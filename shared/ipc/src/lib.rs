#[cfg(feature = "codec")]
mod codec;
#[cfg(feature = "tcp")]
pub mod tcp;

#[cfg(feature = "codec")]
pub use self::codec::Codec;

use bars_config::{
	Aerodrome, Block, BlockState, GeoMap, Icao, Map, Node, Preset, Profile, Ref,
	State, Style,
};
use bars_graph::MapUpdate;

use serde::{Deserialize, Serialize};

pub const PORT: u16 = 21314;
pub const TCP_INIT_BYTE: u8 = 0xba;

pub static WS_PROTOCOL_JSON: &str = "connect.euroscope.stopbars.com+json";
pub static WS_PROTOCOL_POSTCARD: &str =
	"connect.euroscope.stopbars.com+postcard";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Upstream {
	Authenticate {
		token: Option<String>,
	},
	Connect {
		target: ConnectionTarget,
	},
	Subscribe {
		aerodrome: Icao,
		subscribe: bool,
	},
	GraphAction {
		aerodrome: Icao,
		action: GraphAction,
	},
	Reload,
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
	UserMessage {
		message: String,
	},
	Connection {
		target: ConnectionTarget,
	},
	Aerodrome {
		aerodrome: Icao,
		config: Option<AerodromeConfig>,
		state: Option<AerodromeState>,
		updates: Vec<MapUpdate>,
	},
	Pilots {
		callsigns: Vec<String>,
	},
}

#[derive(
	Clone,
	Copy,
	Debug,
	Default,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Deserialize,
	Serialize,
)]
pub enum ConnectionTarget {
	#[default]
	None,
	Network,
	Local,
}

#[derive(
	Clone,
	Copy,
	Debug,
	Default,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Deserialize,
	Serialize,
)]
pub enum AerodromeState {
	#[default]
	None,
	Loading,
	Error,
	Observe,
	Control,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AerodromeConfig {
	pub config: Aerodrome,
	pub geo_map: Option<GeoMap>,
	pub maps: Vec<Map>,
	pub styles: Vec<Style>,
}
