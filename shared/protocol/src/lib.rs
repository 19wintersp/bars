use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type NodeState = bool;

#[derive(
	Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum BlockState {
	Clear,
	Relax,
	Route((String, String)),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Patch {
	pub profile: Option<String>,
	pub nodes: HashMap<String, NodeState>,
	pub blocks: HashMap<String, BlockState>,
}

impl Patch {
	pub fn apply_patch(&mut self, patch: Patch) {
		if let Some(profile) = patch.profile {
			self.profile = Some(profile);
		}

		self.nodes.extend(patch.nodes.into_iter());
		self.blocks.extend(patch.blocks.into_iter());
	}

	pub fn is_empty(&self) -> bool {
		self.profile.is_none() && self.nodes.is_empty() && self.blocks.is_empty()
	}
}

impl Default for Patch {
	fn default() -> Self {
		Self {
			profile: None,
			nodes: HashMap::new(),
			blocks: HashMap::new(),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
	rename_all = "SCREAMING_SNAKE_CASE",
	rename_all_fields = "camelCase",
	tag = "type",
	content = "data"
)]
pub enum Upstream<P = Patch> {
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateUpdate {
	object_id: String,
	state: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
	rename_all = "SCREAMING_SNAKE_CASE",
	rename_all_fields = "camelCase",
	tag = "type",
	content = "data"
)]
pub enum Downstream<P = Patch> {
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
