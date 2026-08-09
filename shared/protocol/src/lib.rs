use serde::{Deserialize, Serialize};

pub type NodeState = bool;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
	rename_all = "SCREAMING_SNAKE_CASE",
	rename_all_fields = "camelCase",
	tag = "type",
	content = "data"
)]
pub enum Upstream<P> {
	Heartbeat,
	#[serde(rename(serialize = "HEARTBEAT"))]
	HeartbeatAck,
	Close,
	GetState,
	StateUpdate {
		object_id: String,
		state: bool,
	},
	MultiStateUpdate {
		updates: Vec<StateUpdate>,
	},
	SharedStateUpdate {
		#[serde(rename = "sharedStatePatch")]
		patch: P,
	},
	GetOnlinePilots,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateUpdate {
	pub object_id: String,
	pub state: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
	rename_all = "SCREAMING_SNAKE_CASE",
	rename_all_fields = "camelCase",
	tag = "type",
	content = "data"
)]
pub enum Downstream<P> {
	Heartbeat,
	HeartbeatAck,
	Close,
	Error {
		message: String,
	},
	ControllerConnect {
		controller_id: String,
	},
	ControllerDisconnect {
		controller_id: String,
	},
	InitialState {
		connection_type: ConnectionType,
		#[serde(rename = "objects")]
		scenery: Vec<SceneryObject>,
		#[serde(rename = "sharedState")]
		patch: P,
	},
	StateUpdate {
		object_id: String,
		state: bool,
		controller_id: String,
	},
	SharedStateUpdate {
		#[serde(rename = "sharedStatePatch")]
		patch: P,
		controller_id: String,
	},
	StateSnapshot {
		objects: Vec<SceneryObject>,
		shared_state: P,
		#[serde(default)]
		controllers: Vec<String>,
		offline: bool,
	},
	StopbarCrossing {
		object_id: String,
	},
	OnlinePilots {
		pilots: Vec<Pilot>,
	},
	#[serde(other)]
	Other,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SceneryObject {
	pub id: String,
	pub state: bool,
	pub timestamp: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
	pub airport: String,
	pub controllers: Vec<String>,
	pub pilots: Vec<String>,
	pub offline: bool,
}

#[derive(
	Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionType {
	Controller,
	Observer,
	Pilot,
	#[serde(other)]
	Other,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Pilot {
	pub cid: String,
	pub callsign: String,
}
