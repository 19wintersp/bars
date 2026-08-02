use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

use bars_config::{Bind, Block, BlockState, Node, Preset, Profile, Ref, ResetTarget, State};
use bars_graph::{
	MapState, MapUpdate, MapUpdateBindCondition, MapUpdateBlockCondition,
	MapUpdateNodeCondition,
};
use bars_ipc::{AerodromeConfig, ConnectionCapacity, GraphAction};

pub struct Aerodrome {
	config: AerodromeConfig,
	profile: AerodromeProfile,
	capacity: ConnectionCapacity,
	nodes: Vec<MapState>,
	binds: Vec<MapState>,
	countdowns: HashMap<ResetTarget, Countdown>,
	actions: Vec<GraphAction>,
}

struct AerodromeProfile {
	profile: Ref<Profile>,
	nodes: Vec<MapUpdateNodeCondition>,
	blocks: Vec<MapUpdateBlockCondition>,
	binds: Vec<MapUpdateBindCondition>,
}

impl Aerodrome {
	pub fn new(config: AerodromeConfig) -> Self {
		let state = MapState {
			current: State::Off,
			pending: State::Off,
		};

		Self {
			profile: AerodromeProfile {
				profile: 0.into(),
				nodes: vec![Default::default(); config.config.nodes.len()],
				blocks: vec![Default::default(); config.config.blocks.len()],
				binds: vec![Default::default(); config.config.binds.len()],
			},
			capacity: ConnectionCapacity::None,
			nodes: vec![state; config.config.nodes.len()],
			binds: vec![state; config.config.binds.len()],
			countdowns: HashMap::new(),
			actions: Vec::new(),
			config,
		}
	}

	pub fn capacity(&self) -> ConnectionCapacity {
		self.capacity
	}

	pub fn set_capacity(&mut self, capacity: ConnectionCapacity) {
		self.capacity = capacity;
	}

	pub fn config(&self) -> &AerodromeConfig {
		&self.config
	}

	pub fn profile_config(&self) -> &Profile {
		&self.config.config.profiles[self.profile.profile.0]
	}

	pub fn profile(&self) -> Ref<Profile> {
		self.profile.profile
	}

	pub fn set_profile(&mut self, profile: Ref<Profile>) {
		if profile.0 < self.config.config.profiles.len() {
			self.act(GraphAction::SetProfile(profile));
		}
	}

	pub fn apply_preset(&mut self, preset: Ref<Preset>) {
		if preset.0 < self.config.config.presets.len() {
			self.act(GraphAction::ApplyPreset(preset));
		}
	}

	pub fn is_node_router(&self, node: Ref<Node>) -> bool {
		self.profile.nodes[node.0].router
	}

	pub fn node_state(&self, node: Ref<Node>) -> MapState {
		self.profile.nodes[node.0]
			.fixed
			.map(MapState::uniform)
			.unwrap_or(self.nodes[node.0])
	}

	pub fn set_node_state(&mut self, node: Ref<Node>, state: State) {
		self.act(GraphAction::SetNodeState(node, state));
	}

	pub fn set_block_state(&mut self, block: Ref<Block>, state: BlockState) {
		self.act(GraphAction::SetBlockState(block, state));
	}

	pub fn insert_route(&mut self, from: Ref<Node>, to: Ref<Node>) {
		self.act(GraphAction::InsertRoute(from, to));
	}

	pub fn bind_state(&self, bind: Ref<Bind>) -> MapState {
		self.profile.binds[bind.0]
			.fixed
			.map(MapState::uniform)
			.unwrap_or(self.binds[bind.0])
	}

	pub fn countdown(&self, target: &ResetTarget) -> Option<&Countdown> {
		self
			.countdowns
			.get(target)
			.filter(|countdown| countdown.is_current())
	}

	fn act(&mut self, action: GraphAction) {
		self.actions.push(action);
	}

	pub(super) fn update(&mut self, update: MapUpdate) {
		match update {
			MapUpdate::Profile {
				profile,
				nodes,
				blocks,
				binds,
			} => {
				self.profile = AerodromeProfile {
					profile,
					nodes,
					blocks,
					binds,
				}
			},
			MapUpdate::NodeState { node, state } => self.nodes[node.0] = state,
			MapUpdate::BindState { bind, state } => self.binds[bind.0] = state,
			MapUpdate::Countdown {
				target,
				length,
				finish,
			} => {
				let Some(finish) = finish
					.duration_since(SystemTime::now())
					.ok()
					.and_then(|duration| Instant::now().checked_add(duration))
				else {
					self.countdowns.remove(&target);
					return
				};
				self.countdowns.insert(target, Countdown { finish, length });
			},
		}
	}

	pub(super) fn take_actions(&mut self) -> Vec<GraphAction> {
		std::mem::take(&mut self.actions)
	}
}

pub struct Countdown {
	pub finish: Instant,
	pub length: Duration,
}

impl Countdown {
	fn is_current(&self) -> bool {
		self.length.as_secs() > 0 && self.finish > Instant::now()
	}

	pub fn remaining(&self) -> Duration {
		self
			.finish
			.checked_duration_since(Instant::now())
			.unwrap_or(Duration::ZERO)
	}
}
