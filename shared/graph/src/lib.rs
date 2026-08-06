use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Debug;
use std::time::{Duration, Instant, SystemTime};

use bars_config::{
	Aerodrome, Bind, BindCondition, BindDependency, Block, BlockCondition,
	BlockNode, BlockRoute, BlockState, Node, NodeCondition, Preset, Profile, Ref,
	ResetCondition, ResetTarget, State,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub const PENDING_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub enum MapUpdate {
	Profile {
		profile: Ref<Profile>,
		nodes: Vec<MapUpdateNodeCondition>,
		blocks: Vec<MapUpdateBlockCondition>,
		binds: Vec<MapUpdateBindCondition>,
	},
	NodeState {
		node: Ref<Node>,
		state: MapState,
	},
	BindState {
		bind: Ref<Bind>,
		state: MapState,
	},
	Countdown {
		target: ResetTarget,
		length: Duration,
		finish: SystemTime,
	},
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct MapUpdateNodeCondition {
	pub fixed: Option<State>,
	pub router: bool,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct MapUpdateBlockCondition {
	pub fixed: bool,
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct MapUpdateBindCondition {
	pub fixed: Option<State>,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct MapState {
	pub current: State,
	pub pending: State,
}

impl MapState {
	pub fn uniform(state: State) -> Self {
		Self {
			current: state,
			pending: state,
		}
	}
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize), serde(default))]
pub struct Patch {
	meta: Option<PatchMeta>,
	#[cfg_attr(
		feature = "serde",
		serde(skip_serializing_if = "Option::is_none")
	)]
	profile: Option<String>,
	nodes: HashMap<String, bool>,
	blocks: HashMap<String, PatchBlock>,
}

impl Patch {
	pub fn is_empty(&self) -> bool {
		self.profile.is_none() && self.nodes.is_empty() && self.blocks.is_empty()
	}

	pub fn is_valid_initial(&self) -> bool {
		self.profile.is_some()
	}
}

#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
struct PatchMeta {
	client: u64,
	serial: u64,
}

#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
enum PatchBlock {
	Clear,
	Relax,
	Route(String, String),
}

pub struct Graph<'a> {
	config: &'a Aerodrome,
	profile_ids: HashMap<&'a str, Ref<Profile>>,
	node_ids: HashMap<&'a str, Ref<Node>>,
	block_ids: HashMap<&'a str, Ref<Block>>,
	element_binds: HashMap<&'a str, Ref<Bind>>,

	client_id: u64,
	serial: u64,

	profile: Ref<Profile>,
	nodes: Vec<GraphNode>,
	blocks: Vec<GraphBlock>,

	nodes_cache: Vec<MapState>,
	binds_cache: Vec<MapState>,

	timeouts: Vec<ResetTarget>,
	crossings: HashMap<&'a str, Vec<ResetTarget>>,

	patch: Patch,
	element_updates: HashMap<String, bool>,
	map_updates: Vec<MapUpdate>,
	/* #[cfg(feature = "debug")]
	debug_elements: HashMap<&'a str, DebugElement>, */
}

/* #[cfg(feature = "debug")]
#[derive(Serialize)]
struct DebugElement {
	coordinates: Vec<[f32; 2]>,
	color: String,
}

#[cfg(feature = "debug")]
impl Graph<'_> {
	// called by ctx, sent to http srv via mpsc
	// patch is added as a Vec externally
	pub fn debug(&self) -> Box<RawValue> {
		#[derive(Serialize)]
		struct Debug<'a> {
			profile: &'a str,
			nodes: HashMap<&'a str, DebugNode<'a>>,
			blocks: HashMap<&'a str, &'a GraphBlock>,
			binds: HashMap<&'a str, MapState>,
			elements: &'a HashMap<String, bool>,
			element_data: &'a HashMap<&'a str, DebugElement>,
		}

		#[derive(Serialize)]
		struct DebugNode<'a> {
			#[serde(flatten)]
			node: &'a GraphNode,
			cache: MapState,
		}

		let debug = Debug {
			profile: &self.config.profiles[self.profile.0].id,
			nodes: self
				.nodes
				.iter()
				.enumerate()
				.map(|(i, node)| {
					(
						self.config.nodes[i].id.as_str(),
						DebugNode {
							node,
							cache: self.nodes_cache[i],
						},
					)
				})
				.collect(),
			blocks: self
				.blocks
				.iter()
				.enumerate()
				.map(|(i, block)| (self.config.blocks[i].id.as_str(), block))
				.collect(),
			binds: self
				.binds_cache
				.iter()
				.enumerate()
				.map(|(i, state)| (self.config.binds[i].id.as_str(), *state))
				.collect(),
			elements: &self.element_updates,
			element_data: &self.debug_elements,
		};
		serde_json::value::to_raw_value(&debug).unwrap()
	}
} */

impl<'a> Graph<'a> {
	/// Creates a new graph.
	///
	/// The client ID should uniquely represent this client among those exchanging
	/// patches.
	///
	/// After creating the graph, the next method call must be either to
	/// `apply_patch` with a profile specified therein, or to `set_profile`.
	pub fn new(config: &'a Aerodrome, client_id: u64) -> Self {
		Self {
			config,
			profile_ids: config
				.profiles
				.iter()
				.enumerate()
				.map(|(i, profile)| (profile.id.as_str(), i.into()))
				.collect(),
			node_ids: config
				.nodes
				.iter()
				.enumerate()
				.map(|(i, node)| (node.id.as_str(), i.into()))
				.collect(),
			block_ids: config
				.blocks
				.iter()
				.enumerate()
				.map(|(i, block)| (block.id.as_str(), i.into()))
				.collect(),
			element_binds: config
				.binds
				.iter()
				.enumerate()
				.flat_map(|(i, bind)| {
					bind
						.elements
						.iter()
						.map(move |element| (element.as_str(), i.into()))
				})
				.collect(),

			client_id,
			serial: 0,

			profile: usize::MAX.into(),
			nodes: Vec::new(),
			blocks: Vec::new(),

			nodes_cache: Vec::new(),
			binds_cache: Vec::new(),

			timeouts: Vec::new(),
			crossings: HashMap::new(),

			patch: Patch::default(),
			element_updates: HashMap::new(),
			map_updates: Vec::new(),
			/* #[cfg(feature = "debug")]
			debug_elements: config
				.elements
				.iter()
				.map(|(id, element)| {
					(
						id.as_str(),
						DebugElement {
							coordinates: element
								.points
								.iter()
								.map(|geo| [geo.lon, geo.lat])
								.collect(),
							color: format!(
								"#{:02x}{:02x}{:02x}",
								element.color.r, element.color.g, element.color.b
							),
						},
					)
				})
				.collect(), */
		}
	}
}

impl Graph<'_> {
	fn profile_changed(&mut self) {
		let initial = self.nodes.is_empty();

		if initial {
			self.nodes = vec![
				GraphNode {
					state: GraphState::new(State::Off),
					binds: Vec::new(),
					block1: None,
					block2: None,
					edges1: Vec::new(),
					edges2: Vec::new(),
					current_child: None,
					pending_child: None,
				};
				self.config.nodes.len()
			];

			self.blocks = vec![
				GraphBlock {
					state: GraphState::new(BlockState::Clear),
					binds: Vec::new(),
					adjacent: Vec::new(),
				};
				self.config.blocks.len()
			];

			for (i, block) in self.config.blocks.iter().enumerate() {
				for node in &block.nodes {
					let node = &mut self.nodes[node.0];
					node.block2 = node.block1.replace(i.into());
				}
			}

			let node_blocks1 = self
				.nodes
				.iter()
				.map(|node| node.block1)
				.collect::<Vec<_>>();

			for (i, node) in self.nodes.iter_mut().enumerate() {
				if let Some((i1, i2)) = node.block1.zip(node.block2) {
					if !self.blocks[i1.0].adjacent.contains(&i2) {
						self.blocks[i1.0].adjacent.push(i2);
						self.blocks[i2.0].adjacent.push(i1);
					}
				}

				let edges = |block: Ref<Block>| -> Vec<_> {
					let nodes = self.config.blocks[block.0]
						.routes
						.iter()
						.filter(|route| route.from.node.0 == i)
						.map(|route| route.to.node)
						.collect::<HashSet<_>>();

					nodes
						.into_iter()
						.map(|node| {
							(
								node,
								if node_blocks1[node.0] == Some(block) {
									NodeSide::Side2
								} else {
									NodeSide::Side1
								},
							)
						})
						.collect()
				};

				if let Some(block1) = node.block1 {
					node.edges1 = edges(block1);
				}

				if let Some(block2) = node.block2 {
					node.edges2 = edges(block2);
				}
			}

			let off = MapState {
				current: State::Off,
				pending: State::Off,
			};
			self.nodes_cache = vec![off; self.config.nodes.len()];
			self.binds_cache = vec![off; self.config.binds.len()];
		}

		for node in &mut self.nodes {
			node.binds.clear();
		}

		for block in &mut self.blocks {
			block.binds.clear();
		}

		self.timeouts.clear();
		self.crossings.clear();

		let profile = &self.config.profiles[self.profile.0];

		let node_resets =
			profile.nodes.iter().enumerate().filter_map(|(i, node)| {
				if let NodeCondition::Direct { reset } = node {
					Some((ResetTarget::Node(i.into()), reset))
				} else {
					None
				}
			});
		let block_resets =
			profile.blocks.iter().enumerate().filter_map(|(i, block)| {
				if let BlockCondition::Router { reset } = block {
					Some((ResetTarget::Block(i.into()), reset))
				} else {
					None
				}
			});
		for (target, reset) in node_resets.chain(block_resets) {
			if reset.timeout > 0 {
				self.timeouts.push(target);
			}

			if let Some(crossing) = reset.crossing.as_ref() {
				self.crossings.entry(crossing).or_default().push(target);
			}
		}

		for (i, bind) in profile.binds.iter().enumerate() {
			if let BindCondition::Bound { block, expression } = bind {
				if let Some(block) = block {
					self.blocks[block.0].binds.push(i.into());
				}

				for conjunction in &expression.disjunction {
					for dependency in
						conjunction.positive.iter().chain(&conjunction.negative)
					{
						if let BindDependency::Node(node) = dependency {
							self.nodes[node.0].binds.push(i.into());
						}
					}
				}
			}
		}

		self.map_updates.push(self.profile_map_update());

		for i in 0..self.config.nodes.len() {
			self.invalidate_node(i.into());
		}
		for i in 0..self.config.binds.len() {
			self.invalidate_bind(i.into());
		}
	}

	fn profile_map_update(&self) -> MapUpdate {
		let profile = &self.config.profiles[self.profile.0];

		MapUpdate::Profile {
			profile: self.profile,
			nodes: profile
				.nodes
				.iter()
				.map(|node| MapUpdateNodeCondition {
					fixed: if let NodeCondition::Fixed { state } = node {
						Some(*state)
					} else {
						None
					},
					router: matches!(node, NodeCondition::Router { .. }),
				})
				.collect(),
			blocks: profile
				.blocks
				.iter()
				.map(|block| MapUpdateBlockCondition {
					fixed: matches!(block, BlockCondition::Fixed { .. }),
				})
				.collect(),
			binds: profile
				.binds
				.iter()
				.map(|bind| MapUpdateBindCondition {
					fixed: if let BindCondition::Fixed { state } = bind {
						Some(*state)
					} else {
						None
					},
				})
				.collect(),
		}
	}

	pub fn profile(&self) -> Ref<Profile> {
		self.profile
	}

	pub fn profiles(&self) -> impl Iterator<Item = &str> {
		self
			.config
			.profiles
			.iter()
			.map(|profile| profile.name.as_str())
	}

	pub fn set_profile(&mut self, profile: Ref<Profile>) {
		if profile.0 >= self.config.profiles.len() {
			tracing::warn!("requested profile {} out of range", profile.0);
			return
		}

		if profile == self.profile {
			return
		}

		self.profile = profile;
		self.profile_changed();

		self.patch.profile = Some(self.config.profiles[profile.0].id.clone());

		for (i, node) in self.config.profiles[self.profile.0]
			.nodes
			.iter()
			.enumerate()
		{
			if let NodeCondition::Direct { reset } = node {
				if reset.timeout > 0 || reset.crossing.is_some() {
					self.set_node_state(i.into(), State::On);
				}
			}
		}

		for i in 0..self.config.binds.len() {
			self.invalidate_bind_dependents(i.into());
		}
	}

	pub fn presets(&self) -> impl Iterator<Item = &str> {
		self
			.config
			.presets
			.iter()
			.filter(|preset| preset.profiles.contains(&self.profile.into()))
			.filter_map(|preset| preset.name.as_ref().map(|name| name.as_str()))
	}

	pub fn apply_preset(&mut self, preset: Ref<Preset>) {
		let Some(preset) = self.config.presets.get(preset.0) else {
			tracing::warn!("requested preset {} out of range", preset.0);
			return
		};

		if !preset.profiles.contains(&self.profile) {
			tracing::warn!("requested preset not listed for current profile");
			return
		}

		for (node, state) in &preset.nodes {
			self.set_node_state(*node, *state);
		}

		for (block, state) in &preset.blocks {
			self.set_block_state_only(*block, *state);
		}
	}

	fn block_state(&self, block: Ref<Block>) -> (&BlockState, &BlockState) {
		match &self.config.profiles[self.profile.0].blocks[block.0] {
			BlockCondition::Fixed { state } => (state, state),
			BlockCondition::Router { .. } => self.blocks[block.0].state.get(),
		}
	}

	fn matching_block_routes(
		&self,
		block: Ref<Block>,
		state: BlockState,
	) -> impl Iterator<Item = &BlockRoute> {
		self.config.blocks[block.0]
			.routes
			.iter()
			.filter(move |route| match state {
				BlockState::Clear => false,
				BlockState::Relax => true,
				BlockState::Route { from, to } => {
					route.from.node == from && route.to.node == to
				},
			})
	}

	fn invalidate_node(&mut self, node: Ref<Node>) {
		let node_meta = &self.nodes[node.0];
		let profile = &self.config.profiles[self.profile.0].nodes[node.0];

		fn state(
			node: Ref<Node>,
			node_meta: &GraphNode,
			profile: &NodeCondition,
			node_state: State,
			mut block_state: impl FnMut(Ref<Block>) -> BlockState,
		) -> State {
			match profile {
				NodeCondition::Fixed { state } => *state,
				NodeCondition::Direct { .. } => node_state,
				NodeCondition::Router { .. } => {
					let routed = [node_meta.block1, node_meta.block2]
						.into_iter()
						.filter_map(std::convert::identity)
						.all(|block| match block_state(block) {
							BlockState::Clear => false,
							BlockState::Relax => true,
							BlockState::Route { from, to } => from == node || to == node,
						});
					if routed { State::Off } else { State::On }
				},
			}
		}

		let state = MapState {
			current: state(
				node,
				node_meta,
				profile,
				*node_meta.state.get().0,
				|block| *self.block_state(block).0,
			),
			pending: state(
				node,
				node_meta,
				profile,
				*node_meta.state.get().1,
				|block| *self.block_state(block).1,
			),
		};

		self.nodes_cache[node.0] = state;
		self.map_updates.push(MapUpdate::NodeState { node, state });

		let is_parent = self.config.nodes[node.0].children > 0
			&& matches!(profile, NodeCondition::Router { .. });
		if let Some((block1, block2)) =
			node_meta.block1.zip(node_meta.block2).filter(|_| is_parent)
		{
			let candidates = |block: Ref<Block>, state: BlockState| {
				self
					.matching_block_routes(block, state)
					.filter_map(|route| {
						if route.from.node == node {
							Some(route.from.child)
						} else if route.to.node == node {
							Some(route.to.child)
						} else {
							None
						}
					})
					.collect::<HashSet<_>>()
			};

			let child = |block1_state: BlockState, block2_state: BlockState| {
				let candidates1 = candidates(block1, block1_state);
				let candidates2 = candidates(block2, block2_state);
				let mut candidates = candidates1.intersection(&candidates2);

				let child = candidates.next();
				candidates
					.next()
					.is_none()
					.then_some(child)
					.flatten()
					.copied()
			};

			let (block1_current, block1_pending) = self.block_state(block1);
			let (block2_current, block2_pending) = self.block_state(block2);

			let current_child = child(*block1_current, *block2_current);
			let pending_child = child(*block1_pending, *block2_pending);

			let node_meta = &mut self.nodes[node.0];
			node_meta.current_child = current_child;
			node_meta.pending_child = pending_child;
		}

		for bind in self.nodes[node.0].binds.clone() {
			self.invalidate_bind(bind);
		}
	}

	fn invalidate_block(&mut self, block: Ref<Block>) {
		for node in &self.config.blocks[block.0].nodes {
			self.invalidate_node(*node);
		}

		let binds = self.blocks[block.0]
			.adjacent
			.iter()
			.chain([&block])
			.flat_map(|block| self.blocks[block.0].binds.iter())
			.copied()
			.collect::<HashSet<_>>();
		for bind in binds {
			self.invalidate_bind(bind);
		}
	}

	fn invalidate_bind(&mut self, bind: Ref<Bind>) {
		let profile = &self.config.profiles[self.profile.0].binds[bind.0];

		fn state(
			profile: &BindCondition,
			mut matching_block_routes: impl FnMut(
				Ref<Block>,
				BlockState,
			) -> Vec<BlockRoute>,
			mut node_matches: impl FnMut(Ref<Node>) -> bool,
			mut block_state: impl FnMut(Ref<Block>) -> BlockState,
			mut block_node: impl FnMut(BlockNode) -> bool,
		) -> State {
			match profile {
				BindCondition::Fixed { state } => *state,
				BindCondition::Bound { block, expression } => {
					let state = if let Some(block) = *block {
						let state = block_state(block);
						if matches!(state, BlockState::Route { .. }) {
							// this could be cached
							matching_block_routes(block, state)
								.into_iter()
								.filter(|route| block_node(route.from) && block_node(route.to))
								.all(|route| {
									expression.evaluate(|dependency| match dependency {
										BindDependency::Node(node) => node_matches(*node),
										BindDependency::Route(check) => &route == check,
									})
								})
						} else {
							expression.evaluate(|dependency| match dependency {
								BindDependency::Node(node) => node_matches(*node),
								BindDependency::Route(_) => state == BlockState::Relax,
							})
						}
					} else {
						expression.evaluate(|dependency| match dependency {
							BindDependency::Node(node) => node_matches(*node),
							BindDependency::Route(_) => {
								tracing::warn!("route dependency without specified block");
								false
							},
						})
					};

					if state { State::On } else { State::Off }
				},
			}
		}

		let state = MapState {
			current: state(
				profile,
				|block, state| {
					self.matching_block_routes(block, state).copied().collect()
				},
				|node| self.nodes_cache[node.0].current == State::On,
				|block| *self.block_state(block).0,
				|block_node| {
					self.nodes[block_node.node.0]
						.current_child
						.is_none_or(|child| child == block_node.child)
				},
			),
			pending: state(
				profile,
				|block, state| {
					self.matching_block_routes(block, state).copied().collect()
				},
				|node| self.nodes_cache[node.0].pending == State::On,
				|block| *self.block_state(block).1,
				|block_node| {
					self.nodes[block_node.node.0]
						.pending_child()
						.is_none_or(|child| child == block_node.child)
				},
			),
		};

		self.binds_cache[bind.0] = state;
		self.map_updates.push(MapUpdate::BindState { bind, state });
	}

	pub fn element_state(&self, element: &str) -> Option<State> {
		self
			.element_binds
			.get(element)
			.map(|bind| self.binds_cache[bind.0].current)
	}

	fn invalidate_bind_dependents(&mut self, bind: Ref<Bind>) {
		for element in &self.config.binds[bind.0].elements {
			self.element_updates.insert(
				element.clone(),
				self.binds_cache[bind.0].pending == State::On,
			);
		}
	}

	pub fn set_node_state(&mut self, node: Ref<Node>, state: State) {
		let profile = &self.config.profiles[self.profile.0].nodes[node.0];

		let timeout = if let NodeCondition::Direct { reset } = profile
			&& reset.timeout > 0
		{
			Some(Duration::from_secs(match state {
				State::On => 0,
				State::Off => reset.timeout as u64,
			}))
		} else {
			None
		};

		self.nodes[node.0].state.set(
			state,
			timeout
				.filter(|duration| !duration.is_zero())
				.map(|timeout| Instant::now() + timeout),
			self.serial,
		);
		if let Some(timeout) = timeout {
			self.map_updates.push(MapUpdate::Countdown {
				target: ResetTarget::Node(node),
				length: timeout,
				finish: SystemTime::now() + timeout,
			});
		}

		self.invalidate_node(node);

		if !matches!(profile, NodeCondition::Fixed { .. }) {
			self
				.patch
				.nodes
				.insert(self.config.nodes[node.0].id.clone(), state == State::On);
		}

		for bind in self.nodes[node.0].binds.clone() {
			self.invalidate_bind_dependents(bind);
		}
	}

	fn set_block_state_only(&mut self, block: Ref<Block>, state: BlockState) {
		// todo! and neighbouring block states, if clearing/relaxing and bordering
		// nodes are fixed off!

		let profile = &self.config.profiles[self.profile.0].blocks[block.0];

		let timeout = if let BlockCondition::Router { reset } = profile
			&& reset.timeout > 0
		{
			Some(Duration::from_secs(match state {
				BlockState::Clear => 0,
				_ => reset.timeout as u64,
			}))
		} else {
			None
		};

		self.blocks[block.0].state.set(
			state,
			timeout
				.filter(|duration| !duration.is_zero())
				.map(|timeout| Instant::now() + timeout),
			self.serial,
		);
		if let Some(timeout) = timeout {
			self.map_updates.push(MapUpdate::Countdown {
				target: ResetTarget::Block(block),
				length: timeout,
				finish: SystemTime::now() + timeout,
			});
		}

		self.invalidate_block(block);

		if !matches!(profile, BlockCondition::Fixed { .. }) {
			self.patch.blocks.insert(
				self.config.blocks[block.0].id.clone(),
				match state {
					BlockState::Clear => PatchBlock::Clear,
					BlockState::Relax => PatchBlock::Relax,
					BlockState::Route { from, to } => PatchBlock::Route(
						self.config.nodes[from.0].id.clone(),
						self.config.nodes[to.0].id.clone(),
					),
				},
			);
		}

		let mut dependents = HashSet::<Ref<Bind>>::new();
		let mut block_dependents = vec![block];

		for node in &self.config.blocks[block.0].nodes {
			let node_meta = &self.nodes[node.0];

			dependents.extend(&node_meta.binds);

			if self.config.nodes[node.0].children > 0 {
				if let Some((block1, block2)) = node_meta.block1.zip(node_meta.block2) {
					if block1 == block {
						block_dependents.push(block2);
					} else if block2 == block {
						block_dependents.push(block1);
					}
				}
			}
		}

		for block in block_dependents {
			dependents.extend(&self.blocks[block.0].binds);
		}

		for bind in dependents {
			self.invalidate_bind_dependents(bind);
		}
	}

	pub fn set_block_state(&mut self, block: Ref<Block>, state: BlockState) {
		if matches!(state, BlockState::Clear | BlockState::Relax) {
			fn add_block<I: Iterator<Item = Ref<Block>>>(
				block: Ref<Block>,
				blocks: &mut Vec<Ref<Block>>,
				neighbours: &impl Fn(Ref<Block>) -> I,
			) {
				for block in neighbours(block) {
					if !blocks.contains(&block) {
						blocks.push(block);
						add_block(block, blocks, neighbours);
					}
				}
			}

			let mut blocks = vec![block];
			add_block(block, &mut blocks, &|block| {
				self.config.blocks[block.0]
					.nodes
					.iter()
					.filter(|node| {
						matches!(
							self.config.profiles[self.profile.0].nodes[node.0],
							NodeCondition::Fixed { state: State::Off }
						)
					})
					.map(|node| &self.nodes[node.0])
					.flat_map(|node| [node.block1, node.block2])
					.filter_map(std::convert::identity)
			});

			for block in blocks {
				self.set_block_state_only(block, state);
			}
		} else {
			self.set_block_state_only(block, state);
		}
	}

	pub fn insert_route(&mut self, from: Ref<Node>, to: Ref<Node>) {
		use NodeSide::*;

		let is_router = |node: Ref<Node>| {
			matches!(
				self.config.profiles[self.profile.0].nodes[node.0],
				NodeCondition::Router { .. },
			)
		};
		if !is_router(from) || !is_router(to) {
			tracing::warn!("refusing to set route between non-router nodes");
			return
		}

		let mut queue = VecDeque::from([(from, Side1, 0), (from, Side2, 0)]);
		let mut seen = HashSet::from([(from, Side1), (from, Side2)]);
		let mut seen_again = HashSet::new();
		let mut prev = HashMap::new();

		let mut route = None::<Vec<_>>;

		while let Some((node, side, dist)) = queue.pop_front() {
			let (sticky, empty) =
				match self.config.profiles[self.profile.0].nodes[node.0] {
					NodeCondition::Direct { .. }
					| NodeCondition::Fixed { state: State::On } => (true, false),
					NodeCondition::Fixed { state: State::Off } => (false, true),
					NodeCondition::Router { sticky } => (sticky, false),
				};

			if node == to {
				if route.is_none() {
					let mut node = Some((node, side));
					let route = route.get_or_insert_default();

					let mut i = 0;

					while let Some(item) = node {
						route.push(item);
						node = prev.get(&item).copied();

						i += 1;
						if i > 1000 {
							tracing::error!("routing error: overflow");
							tracing::debug!("{node:?} {prev:?} {seen:?} {queue:?}");
							return
						}
					}

					if dist <= 1 {
						break
					}
				} else {
					tracing::warn!("routing error: multiple candidates");
					return
				}
			} else if !sticky {
				let edges = match side {
					Side1 => &self.nodes[node.0].edges1,
					Side2 => &self.nodes[node.0].edges2,
				};

				for (next_node, next_side) in edges {
					let next_key = (*next_node, *next_side);
					let next = (*next_node, *next_side, dist + !empty as usize);

					if seen.insert(next_key) {
						prev.insert(next_key, (node, side));
						if empty {
							queue.push_front(next);
						} else {
							queue.push_back(next);
						}
					} else {
						seen_again.insert(next_key);
					}
				}
			}
		}

		if let Some(route) = route {
			if route[..route.len() - 1]
				.iter()
				.any(|key| seen_again.contains(key))
			{
				tracing::warn!("routing error: traverses loop");
				return
			}

			for pair in route.windows(2) {
				let [(node2, _), (node1, side1)] = pair else {
					unreachable!()
				};

				let block = match side1 {
					Side1 => self.nodes[node1.0].block1,
					Side2 => self.nodes[node1.0].block2,
				};
				if let Some(block) = block {
					self.set_block_state_only(
						block,
						BlockState::Route {
							from: *node1,
							to: *node2,
						},
					);
				} else {
					tracing::error!("routed node has no block");
				}
			}
		}
	}

	fn do_reset(&mut self, target: ResetTarget) {
		match target {
			ResetTarget::Node(r) => self.set_node_state(r, State::On),
			ResetTarget::Block(r) => self.set_block_state(r, BlockState::Clear),
		}
	}

	pub fn update_timers(&mut self) {
		let targets = self
			.timeouts
			.iter()
			.filter(|timeout| {
				match timeout {
					ResetTarget::Node(node) => &mut self.nodes[node.0].state.timeout,
					ResetTarget::Block(block) => &mut self.blocks[block.0].state.timeout,
				}
				.take_if(|t| *t < Instant::now())
				.is_some()
			})
			.copied()
			.collect::<Vec<_>>();
		for target in targets {
			self.do_reset(target);
		}
	}

	pub fn update_crossing(&mut self, object: &str) {
		if let Some(dependents) = self.crossings.get(object) {
			for dependent in dependents.clone() {
				self.do_reset(dependent);
			}
		}
	}

	pub fn apply_patch(&mut self, patch: &Patch) {
		let serial = patch
			.meta
			.as_ref()
			.filter(|meta| meta.client == self.client_id)
			.map(|meta| meta.serial);

		if let Some(id) = &patch.profile {
			if let Some(i) = self.profile_ids.get(id.as_str()).copied() {
				self.profile = i.into();
				self.profile_changed();
			} else {
				tracing::error!("failed to set unknown profile {id}");
			}
		}

		for (id, state) in &patch.nodes {
			if let Some(i) = self.node_ids.get(id.as_str()).copied() {
				let state = if *state { State::On } else { State::Off };
				self.nodes[i.0].state.update(state, serial);
				self.invalidate_node(i);
			} else {
				tracing::error!("failed to set unknown node {id}");
			}
		}

		for (id, state) in &patch.blocks {
			if let Some(i) = self.block_ids.get(id.as_str()).copied() {
				let Some(state) = (match state {
					PatchBlock::Clear => Some(BlockState::Clear),
					PatchBlock::Relax => Some(BlockState::Relax),
					PatchBlock::Route(from, to) => {
						let from = self.node_ids.get(from.as_str()).copied();
						let to = self.node_ids.get(to.as_str()).copied();
						from.zip(to).map(|(from, to)| BlockState::Route {
							from: from.into(),
							to: to.into(),
						})
					},
				}) else {
					tracing::error!("failed to set route with unknown nodes in {id}");
					continue
				};

				self.blocks[i.0].state.update(state, serial);
				self.invalidate_block(i);
			} else {
				tracing::error!("failed to set unknown block {id}");
			}
		}
	}

	pub fn take_patch(&mut self) -> Option<Patch> {
		if self.patch.is_empty() {
			None
		} else {
			self.serial += 1;
			Some(std::mem::replace(
				&mut self.patch,
				Patch {
					meta: Some(PatchMeta {
						client: self.client_id,
						serial: self.serial,
					}),
					..Patch::default()
				},
			))
		}
	}

	pub fn take_element_updates(&mut self) -> HashMap<String, bool> {
		std::mem::take(&mut self.element_updates)
	}

	pub fn take_map_updates(&mut self) -> Vec<MapUpdate> {
		std::mem::take(&mut self.map_updates)
	}

	pub fn initial_map_updates(&self) -> Vec<MapUpdate> {
		let now = Instant::now();
		let profile = &self.config.profiles[self.profile.0];

		[self.profile_map_update()]
			.into_iter()
			.chain(
				self
					.nodes_cache
					.iter()
					.copied()
					.enumerate()
					.map(|(i, state)| MapUpdate::NodeState {
						node: i.into(),
						state,
					}),
			)
			.chain(
				self
					.binds_cache
					.iter()
					.copied()
					.enumerate()
					.map(|(i, state)| MapUpdate::BindState {
						bind: i.into(),
						state,
					}),
			)
			.chain(
				self
					.timeouts
					.iter()
					.copied()
					.filter_map(|target| {
						match target {
							ResetTarget::Node(node) => self.nodes[node.0].state.timeout,
							ResetTarget::Block(block) => self.blocks[block.0].state.timeout,
						}
						.filter(|timeout| timeout > &now)
						.map(|timeout| (target, timeout))
					})
					.map(|(target, timeout)| MapUpdate::Countdown {
						target,
						length: Duration::from_secs(match target {
							ResetTarget::Node(node) => match profile.nodes[node.0] {
								NodeCondition::Direct {
									reset: ResetCondition { timeout, .. },
								} => timeout,
								_ => 0,
							},
							ResetTarget::Block(block) => match profile.blocks[block.0] {
								BlockCondition::Router {
									reset: ResetCondition { timeout, .. },
								} => timeout,
								_ => 0,
							},
						} as u64),
						finish: SystemTime::now() + (timeout - now),
					}),
			)
			.collect()
	}
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum NodeSide {
	Side1,
	Side2,
}

#[derive(Clone)]
// #[cfg_attr(feature = "debug", derive(Serialize))]
struct GraphNode {
	state: GraphState<State>,
	// #[cfg_attr(feature = "debug", serde(skip))]
	binds: Vec<Ref<Bind>>,
	// #[cfg_attr(feature = "debug", serde(skip))]
	block1: Option<Ref<Block>>,
	// #[cfg_attr(feature = "debug", serde(skip))]
	block2: Option<Ref<Block>>,
	// #[cfg_attr(feature = "debug", serde(skip))]
	edges1: Vec<(Ref<Node>, NodeSide)>,
	// #[cfg_attr(feature = "debug", serde(skip))]
	edges2: Vec<(Ref<Node>, NodeSide)>,
	current_child: Option<usize>,
	pending_child: Option<usize>,
}

impl GraphNode {
	fn pending_child(&self) -> Option<usize> {
		/* if self
			.state
			.pending
			.as_ref()
			.is_some_and(|pending| !pending.is_superseded(None))
		{
			self.pending_child
		} else {
			self.current_child
		} */

		// todo! fixme: this needs reconsideration... the above does not work
		// because a neighbouring block could be pending without ourselves
		self.pending_child
	}
}

#[derive(Clone)]
// #[cfg_attr(feature = "debug", derive(Serialize))]
struct GraphBlock {
	state: GraphState<BlockState>,
	// #[cfg_attr(feature = "debug", serde(skip))]
	binds: Vec<Ref<Bind>>,
	// #[cfg_attr(feature = "debug", serde(skip))]
	adjacent: Vec<Ref<Block>>,
}

#[derive(Clone)]
struct GraphState<T> {
	current: T,
	timeout: Option<Instant>,
	pending: Option<GraphPendingState<T>>,
}

/* #[cfg(feature = "debug")]
impl<T: Debug> Serialize for GraphState<T> {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		let mut s = s.serialize_struct("State", 2)?;
		s.serialize_field("current", &format!("{:?}", self.current))?;
		s.serialize_field("pending", &self.pending)?;
		s.end()
	}
} */

impl<T> GraphState<T> {
	fn new(current: T) -> Self {
		Self {
			current,
			timeout: None,
			pending: None,
		}
	}

	fn get(&self) -> (&T, &T) {
		(
			&self.current,
			self
				.pending
				.as_ref()
				.filter(|pending| !pending.is_superseded(None))
				.map(|pending| &pending.state)
				.unwrap_or(&self.current),
		)
	}

	fn set(&mut self, state: T, timeout: Option<Instant>, serial: u64) {
		self.pending = Some(GraphPendingState {
			state,
			timeout,
			serial,
			time_set: Instant::now(),
		});
	}

	fn update(&mut self, state: T, serial: Option<u64>) {
		self.current = state;
		self.timeout = self
			.pending
			.as_ref()
			.filter(|pending| serial.is_some_and(|serial| serial == pending.serial))
			.and_then(|pending| pending.timeout);
		self
			.pending
			.take_if(|pending| pending.is_superseded(serial));
	}
}

#[derive(Clone)]
struct GraphPendingState<T> {
	state: T,
	timeout: Option<Instant>,
	serial: u64,
	time_set: Instant,
}

/* #[cfg(feature = "debug")]
impl<T: Debug> Serialize for GraphPendingState<T> {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		let ts = SystemTime::now() - self.time_set.elapsed();
		let tr = ts.duration_since(SystemTime::UNIX_EPOCH).unwrap();

		let mut s = s.serialize_struct("PendingState", 3)?;
		s.serialize_field("state", &format!("{:?}", self.state))?;
		s.serialize_field("serial", &self.serial)?;
		s.serialize_field("time_set", &tr.as_secs())?;
		s.end()
	}
} */

impl<T> GraphPendingState<T> {
	fn is_superseded(&self, serial: Option<u64>) -> bool {
		serial.is_some_and(|serial| serial >= self.serial)
			|| self.time_set.elapsed() > PENDING_TIMEOUT
	}
}
