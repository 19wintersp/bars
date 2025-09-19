use crate::ipc::{Channel, Downstream, Upstream};
use crate::ActivityState;

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use bars_config::{
	BlockCondition, BlockRoute, BlockState, EdgeCondition, EdgeState,
	ElementCondition, NodeCondition, NodeState, ResetCondition,
};

use bars_protocol::{BlockState as IpcBlockState, Patch};

use anyhow::Result;

use tracing::{debug, warn};

pub struct Client {
	channel: Channel,
	aerodromes: HashMap<String, Aerodrome>,
}

impl Client {
	pub fn new(mut channel: Channel) -> Result<Self> {
		channel.send(Upstream::Init)?;

		Ok(Self {
			channel,
			aerodromes: HashMap::new(),
		})
	}

	pub fn disconnect(self) {}

	pub fn tick(&mut self) -> Result<Vec<String>> {
		let mut user_messages = Vec::new();

		while let Some(message) = self.channel.recv()? {
			match message {
				Downstream::Config { data } => {
					let aerodrome = bars_config::Aerodrome::decode(&data)?;

					self
						.aerodromes
						.entry(aerodrome.icao.clone())
						.or_insert_with(|| Aerodrome::new(aerodrome));
				},
				Downstream::Control { icao, control } => {
					if let Some(aerodrome) = self.aerodromes.get_mut(&icao) {
						aerodrome.state = if control {
							ActivityState::Controlling
						} else {
							ActivityState::Observing
						};
					}
				},
				Downstream::Patch { icao, patch } => {
					if let Some(aerodrome) = self.aerodromes.get_mut(&icao) {
						aerodrome.apply_patch(patch);
					}
				},
				Downstream::Aircraft { icao, aircraft } => {
					if let Some(aerodrome) = self.aerodromes.get_mut(&icao) {
						aerodrome.aircraft = HashSet::from_iter(aircraft);
					}
				},
				Downstream::Error {
					icao,
					message,
					disconnect,
				} => {
					user_messages.push(format!(
						"server: {icao}: {}",
						message.as_ref().map(|s| s.as_str()).unwrap_or("error"),
					));

					if disconnect {
						self.set_tracking(icao, false)?;
					}
				},
			}
		}

		for (icao, aerodrome) in &mut self.aerodromes {
			aerodrome.tick();

			let (patch, scenery) = aerodrome.take_pending();

			if !patch.is_empty() {
				self.channel.send(Upstream::Patch {
					icao: icao.clone(),
					patch,
				})?;
			}

			if !scenery.is_empty() {
				self.channel.send(Upstream::Scenery {
					icao: icao.clone(),
					scenery,
				})?;
			}
		}

		Ok(user_messages)
	}

	pub fn set_tracking(&mut self, icao: String, track: bool) -> Result<()> {
		if !track {
			self.aerodromes.remove(&icao);
		}

		self.channel.send(Upstream::Track { icao, track })
	}

	pub fn set_controlling(&mut self, icao: String, control: bool) -> Result<()> {
		if self.aerodromes.contains_key(&icao) {
			self.channel.send(Upstream::Control { icao, control })
		} else {
			warn!("attempted to un/control untracked aerodrome");
			Ok(())
		}
	}

	pub fn aerodrome(&self, icao: &String) -> Option<&Aerodrome> {
		self.aerodromes.get(icao)
	}

	pub fn aerodrome_mut(&mut self, icao: &String) -> Option<&mut Aerodrome> {
		self.aerodromes.get_mut(icao)
	}
}

#[derive(Clone)]
struct State<T> {
	current: T,
	pending: Option<T>,
}

impl<T> State<T> {
	fn state(&self) -> &T {
		self.pending.as_ref().unwrap_or(&self.current)
	}
}

pub struct Aerodrome {
	config: bars_config::Aerodrome,
	state: ActivityState,

	profile: usize,

	node_ids: HashMap<String, usize>,
	block_ids: HashMap<String, usize>,

	node_conns: Vec<[Vec<(usize, bool)>; 2]>,
	node_blocks: Vec<[usize; 2]>,
	children: HashMap<usize, Vec<usize>>,

	nodes: Vec<State<bool>>,
	blocks: Vec<State<BlockState>>,

	aircraft: HashSet<String>,

	pending_patch: Patch,
	pending_nodes: Vec<usize>,
	previous_edges: Vec<bool>,
	node_dependencies: Vec<Vec<usize>>,
	edge_dependencies: Vec<Vec<usize>>,

	node_timers: Vec<(usize, Instant)>,
	block_timers: Vec<(usize, Instant)>,
}

impl Aerodrome {
	fn new(config: bars_config::Aerodrome) -> Self {
		let mut this = Self {
			config,
			state: ActivityState::None,
			profile: 0,
			node_ids: HashMap::new(),
			block_ids: HashMap::new(),
			node_conns: Vec::new(),
			node_blocks: Vec::new(),
			children: HashMap::new(),
			nodes: Vec::new(),
			blocks: Vec::new(),
			aircraft: HashSet::new(),
			pending_patch: Default::default(),
			previous_edges: Vec::new(),
			pending_nodes: Vec::new(),
			node_dependencies: Vec::new(),
			edge_dependencies: Vec::new(),
			node_timers: Vec::new(),
			block_timers: Vec::new(),
		};

		let mut borders = vec![0; this.config.nodes.len()];
		this
			.node_conns
			.resize(this.config.nodes.len(), [Vec::new(), Vec::new()]);
		this.node_blocks.resize(this.config.nodes.len(), [0; 2]);

		for (i, node) in this.config.nodes.iter().enumerate() {
			this.node_ids.insert(node.id.clone(), i);

			if let Some(parent) = node.parent {
				this.children.entry(parent.0).or_default().push(i);
			}
		}

		for (i, block) in this.config.blocks.iter().enumerate() {
			this.block_ids.insert(block.id.clone(), i);

			let conns = block
				.nodes
				.iter()
				.copied()
				.map(|node| (node, borders[node.0] > 0))
				.collect::<Vec<_>>();

			for node in block.nodes.iter().copied() {
				let node_borders = &mut borders[node.0];

				this.node_blocks[node.0][1] = i;
				this.node_blocks[node.0][*node_borders] = i;

				this.node_conns[node.0][*node_borders].extend(
					conns
						.iter()
						.filter(|(node_, _)| {
							node_.0 != node.0
								&& !block.non_routes.contains(&BlockRoute {
									from: node,
									to: *node_,
								})
						})
						.map(|(node, side)| (node.0, *side)),
				);

				*node_borders += 1;
			}
		}

		this
			.node_dependencies
			.resize(this.config.nodes.len(), Vec::new());
		this
			.edge_dependencies
			.resize(this.config.edges.len(), Vec::new());

		for (i, element) in this.config.elements.iter().enumerate() {
			match element.condition {
				ElementCondition::Fixed(_) => (),
				ElementCondition::Node(node) => this.node_dependencies[node.0].push(i),
				ElementCondition::Edge(edge) => this.edge_dependencies[edge.0].push(i),
			}
		}

		this.set_default_state(false);

		this
	}

	fn bs_ipc_to_conf(&self, state: IpcBlockState) -> Option<BlockState> {
		Some(match state {
			IpcBlockState::Clear => BlockState::Clear,
			IpcBlockState::Relax => BlockState::Relax,
			IpcBlockState::Route((a, b)) => BlockState::Route((
				(*self.node_ids.get(&a)?).into(),
				(*self.node_ids.get(&b)?).into(),
			)),
		})
	}

	fn bs_conf_to_ipc(&self, state: &BlockState) -> IpcBlockState {
		match state {
			BlockState::Clear => IpcBlockState::Clear,
			BlockState::Relax => IpcBlockState::Relax,
			BlockState::Route((a, b)) => IpcBlockState::Route((
				self.config.nodes[a.0].id.clone(),
				self.config.nodes[b.0].id.clone(),
			)),
		}
	}

	fn apply_patch(&mut self, patch: Patch) {
		if let Some(profile) = patch.profile {
			if let Some(i) = self.config.profiles.iter().position(|p| p.id == profile)
			{
				self.profile = i;

				self.node_timers.clear();
				self.block_timers.clear();
			} else {
				warn!("requested to set unknown profile");
			}
		}

		for (id, state) in patch.nodes {
			if let Some(i) = self.node_ids.get(&id).copied() {
				self.nodes[i].current = state;
				if self.nodes[i].pending == Some(state) {
					self.nodes[i].pending = None;
				} else {
					self.node_timers.retain(|(node, _)| node != &i);
				}
			}
		}

		for (id, state) in patch.blocks {
			if let Some(i) = self.block_ids.get(&id).copied() {
				let Some(state) = self.bs_ipc_to_conf(state) else {
					continue
				};

				self.blocks[i].current = state;
				if self.blocks[i].pending == Some(state) {
					self.blocks[i].pending = None;
				} else {
					self.block_timers.retain(|(block, _)| block != &i);
				}
			}
		}
	}

	fn tick(&mut self) {
		let now = Instant::now();

		while self.node_timers.first().map(|(_, time)| time < &now) == Some(true) {
			let (node, _) = self.node_timers.remove(0);
			self.set_node(node, true);
		}

		while self.block_timers.first().map(|(_, time)| time < &now) == Some(true) {
			let (block, _) = self.block_timers.remove(0);
			self.set_block(block, BlockState::Clear);
		}
	}

	fn take_pending(&mut self) -> (Patch, HashMap<String, bool>) {
		let next_edges = self.calculate_edges();

		let patch = std::mem::take(&mut self.pending_patch);
		let nodes = std::mem::take(&mut self.pending_nodes);
		let mut scenery = HashMap::new();

		if patch.profile.is_some() {
			for element in &self.config.elements {
				scenery.insert(
					element.id.clone(),
					match element.condition {
						ElementCondition::Fixed(state) => state,
						ElementCondition::Edge(edge) => next_edges[edge.0],
						ElementCondition::Node(node) => *self.nodes[node.0].state(),
					},
				);
			}
		} else {
			for i in nodes {
				for element in &self.node_dependencies[i] {
					scenery.insert(
						self.config.elements[*element].id.clone(),
						*self.nodes[i].state(),
					);
				}
			}

			for (i, (prev, next)) in
				next_edges.iter().zip(&self.previous_edges).enumerate()
			{
				if prev != next {
					for element in &self.edge_dependencies[i] {
						scenery.insert(self.config.elements[*element].id.clone(), *next);
					}
				}
			}
		}

		self.previous_edges = next_edges;

		(patch, scenery)
	}

	fn calculate_edges(&self) -> Vec<bool> {
		(0..self.config.edges.len())
			.map(|i| self.edge_state(i))
			.collect()
	}

	fn set_default_state(&mut self, patch: bool) {
		self.nodes = Vec::with_capacity(self.config.nodes.len());
		self.blocks = vec![
			State {
				current: BlockState::Clear,
				pending: None,
			};
			self.config.blocks.len()
		];

		for i in 0..self.config.nodes.len() {
			self.nodes.push(State {
				current: match self.config.profiles[self.profile].nodes[i] {
					NodeCondition::Fixed { state } => state == NodeState::On,
					NodeCondition::Direct { reset } => reset != ResetCondition::None,
					_ => true,
				},
				pending: None,
			});
		}

		if patch {
			self.pending_patch.nodes =
				HashMap::from_iter(self.nodes.iter().enumerate().map(
					|(node, state)| (self.config.nodes[node].id.clone(), *state.state()),
				));
			self.pending_nodes = (0..self.nodes.len()).collect();
			self.pending_patch.blocks = HashMap::from_iter(
				self.blocks.iter().enumerate().map(|(block, state)| {
					(
						self.config.blocks[block].id.clone(),
						self.bs_conf_to_ipc(state.state()),
					)
				}),
			);
		} else {
			self.previous_edges = self.calculate_edges();
		}

		self.node_timers.clear();
		self.block_timers.clear();
	}

	fn set_node_state(&mut self, node: usize, state: bool) {
		self.nodes[node].pending = Some(state);
		self
			.pending_patch
			.nodes
			.insert(self.config.nodes[node].id.clone(), state);
		self.pending_nodes.push(node);

		self.node_timers.retain(|(node_, _)| node_ != &node);

		if !state {
			if let NodeCondition::Direct {
				reset: ResetCondition::TimeSecs(secs),
			} = self.config.profiles[self.profile].nodes[node]
			{
				let deadline = Instant::now() + Duration::from_secs(secs as u64);
				self.node_timers.push((node, deadline));
			}
		}
	}

	fn set_block_state(&mut self, block: usize, state: BlockState) {
		self.blocks[block].pending = Some(state);
		self.pending_patch.blocks.insert(
			self.config.blocks[block].id.clone(),
			self.bs_conf_to_ipc(&state),
		);

		self.block_timers.retain(|(block_, _)| block_ != &block);

		if state != BlockState::Clear {
			if let BlockCondition {
				reset: ResetCondition::TimeSecs(secs),
			} = self.config.profiles[self.profile].blocks[block]
			{
				let deadline = Instant::now() + Duration::from_secs(secs as u64);
				self.block_timers.push((block, deadline));
			}
		}
	}

	pub fn state(&self) -> ActivityState {
		self.state
	}

	pub fn profile(&self) -> usize {
		self.profile
	}

	pub fn set_profile(&mut self, i: usize) {
		if i >= self.config.profiles.len() {
			return
		}

		self.profile = i;
		self.pending_patch.profile = Some(self.config.profiles[i].id.clone());
		self.set_default_state(true);
	}

	pub fn apply_preset(&mut self, i: usize) {
		if i >= self.config.profiles[self.profile].presets.len() {
			return
		}

		let preset = &self.config.profiles[self.profile].presets[i];
		let mut nodes = HashMap::new();
		let mut blocks = HashMap::new();

		for (node, state) in &preset.nodes {
			if node.0 < self.nodes.len() {
				let state = *state == NodeState::On;
				self.nodes[node.0].pending = Some(state);
				nodes.insert(self.config.nodes[node.0].id.clone(), state);
			}
		}

		for (block, state) in &preset.blocks {
			if block.0 < self.blocks.len() {
				self.blocks[block.0].pending = Some(*state);
				blocks.insert(
					self.config.blocks[block.0].id.clone(),
					self.bs_conf_to_ipc(state),
				);
			}
		}

		self.pending_patch.nodes = nodes;
		self.pending_nodes = preset.nodes.iter().map(|(i, _)| i.0).collect();
		self.pending_patch.blocks = blocks;

		self.node_timers.clear();
		self.block_timers.clear();
	}

	pub fn config(&self) -> &bars_config::Aerodrome {
		&self.config
	}

	pub fn is_pilot_enabled(&self, callsign: &str) -> bool {
		self.aircraft.contains(callsign)
	}

	pub fn node_state(&self, node: usize) -> bool {
		match self.config.profiles[self.profile].nodes[node] {
			NodeCondition::Fixed { state } => state == NodeState::On,
			NodeCondition::Direct { .. } => *self.nodes[node].state(),
			NodeCondition::Router { .. } => {
				let blocks = &self.node_blocks[node];
				blocks
					.iter()
					.any(|block| match self.blocks[*block].state() {
						BlockState::Clear => true,
						BlockState::Relax => false,
						BlockState::Route((a, b)) => a.0 != node && b.0 != node,
					})
			},
		}
	}

	fn route_candidates(&self, block: usize) -> Vec<(usize, usize)> {
		let BlockState::Route((ap, bp)) = *self.blocks[block].state() else {
			return vec![]
		};
		let (ap, bp) = (ap.0, bp.0);

		let mut routes = Vec::new();

		let ao = vec![ap];
		let bo = vec![bp];
		let ac = self.children.get(&ap).unwrap_or(&ao);
		let bc = self.children.get(&bp).unwrap_or(&bo);

		let non_routes = &self.config.blocks[block].non_routes;

		for a in ac.iter().copied() {
			for b in bc.iter().copied() {
				if !non_routes.contains(&BlockRoute {
					from: a.into(),
					to: b.into(),
				}) {
					routes.push((a, b));
				}
			}
		}

		routes
	}

	pub fn edge_state(&self, edge: usize) -> bool {
		match &self.config.profiles[self.profile].edges[edge] {
			EdgeCondition::Fixed { state } => *state == EdgeState::On,
			EdgeCondition::Direct { nodes } => {
				nodes.evaluate(&|node| {
					if self.node_state(node.0) {
						NodeState::On
					} else {
						NodeState::Off
					}
				}) == EdgeState::On
			},
			EdgeCondition::Router { block, ref routes } => {
				match *self.blocks[block.0].state() {
					BlockState::Clear => false,
					BlockState::Relax => true,
					BlockState::Route((ap, bp)) => {
						let (ap, bp) = (ap.0, bp.0);

						let cands = self.route_candidates(block.0);
						match cands.len() {
							0 => return false,
							1 => {
								let (a, b) = cands[0];
								return routes.contains(&BlockRoute {
									from: a.into(),
									to: b.into(),
								})
							},
							_ => (),
						}

						// this implementation works for the most common cases only; it does
						// not support the specification in full

						let mut matches = (HashSet::new(), HashSet::new());

						//let ao = vec![ap];
						//let ac = self.children.get(&ap).unwrap_or(&ao);
						for BlockRoute { from: a, to: b } in routes.iter().copied() {
							//let (a, b) = if ac.contains(&a) { (a, b) } else { (b, a) };

							matches.0.insert(a.0);
							matches.1.insert(b.0);
						}

						let mut cands = (
							HashSet::<usize>::from_iter(cands.iter().map(|r| r.0)),
							HashSet::<usize>::from_iter(cands.iter().map(|r| r.1)),
						);

						for (parent, cands) in [(ap, &mut cands.0), (bp, &mut cands.1)] {
							let [b1, b2] = self.node_blocks[parent];
							let adjacent = if b1 != block.0 { b1 } else { b2 };

							match *self.blocks[adjacent].state() {
								BlockState::Clear => (),
								BlockState::Relax => cands.clear(),
								BlockState::Route((a, b)) => {
									let points = self.route_candidates(adjacent).into_iter();

									if a.0 == parent {
										*cands = HashSet::from_iter(points.map(|r| r.0));
									} else if b.0 == parent {
										*cands = HashSet::from_iter(points.map(|r| r.1));
									}
								},
							}
						}

						cands.0.is_subset(&matches.0) && cands.1.is_subset(&matches.1)
					},
				}
			},
		}
	}

	pub fn set_block(&mut self, block: usize, state: BlockState) {
		if block >= self.blocks.len() {
			return
		}

		let mut blocks = vec![block];
		let mut visited = HashSet::new();

		while let Some(block) = blocks.pop() {
			if !visited.insert(block) {
				continue
			}

			self.set_block_state(block, state);

			blocks.extend(
				self.config.blocks[block]
					.nodes
					.iter()
					.filter(|node| {
						self.config.profiles[self.profile].nodes[node.0]
							== NodeCondition::Fixed {
								state: NodeState::Off,
							}
					})
					.flat_map(|node| self.node_blocks[node.0]),
			);
		}
	}

	pub fn set_route(&mut self, (orgn, dest): (usize, usize)) {
		if !matches!(
			self.config.profiles[self.profile].nodes[orgn],
			NodeCondition::Router { .. }
		) || !matches!(
			self.config.profiles[self.profile].nodes[dest],
			NodeCondition::Router { .. }
		) {
			return
		}

		// todo: if orgn/dest are in same block, and the same route is currently
		// selected, clear the block.

		let mut nodes = VecDeque::from([(orgn, false, 0), (orgn, true, 0)]);
		let mut visited = HashSet::from([(orgn, false), (orgn, true)]);
		let mut chain = HashMap::new();
		let mut list: Option<Vec<(usize, bool)>> = None;
		let mut revisited = HashSet::new();

		while let Some((node, direction, distance)) = nodes.pop_front() {
			let condition = self.config.profiles[self.profile].nodes[node];
			if condition
				== (NodeCondition::Fixed {
					state: NodeState::On,
				}) {
				continue
			}

			let transparent = condition
				== NodeCondition::Fixed {
					state: NodeState::Off,
				};

			if node == dest {
				if list.is_none() {
					let mut prev = Some((node, direction));
					let list = list.get_or_insert_default();

					let mut i = 0;

					while let Some(item) = prev {
						i += 1;
						list.push(item);
						prev = chain.get(&item).copied();

						if i > 1000 {
							warn!("overflow {chain:?} {visited:?} {nodes:?}");
							return
						}
					}

					if distance > 1 {
						continue
					} else {
						break
					}
				} else {
					debug!("routing error");
					return
				}
			}

			for (next_node, next_dir) in &self.node_conns[node][direction as usize] {
				let next_key = (*next_node, !next_dir);
				let next = (*next_node, !next_dir, distance + !transparent as usize);

				if visited.insert(next_key) {
					chain.insert(next_key, (node, direction));
					if transparent {
						nodes.push_front(next);
					} else {
						nodes.push_back(next);
					}
				} else {
					revisited.insert(next_key);
				}
			}
		}

		if let Some(list) = list {
			if list[..list.len() - 1]
				.iter()
				.any(|key| revisited.contains(key))
			{
				debug!("routing error");
				return
			}

			for pair in list.windows(2) {
				let [(node2, _), (node1, direction1)] = pair else {
					unreachable!()
				};

				let block = self.node_blocks[*node1][*direction1 as usize];
				self.set_block_state(
					block,
					BlockState::Route(((*node1).into(), (*node2).into())),
				);
			}
		}
	}

	pub fn set_node(&mut self, node: usize, state: bool) {
		if node >= self.nodes.len() {
			return
		}

		if let NodeCondition::Direct { .. } =
			self.config.profiles[self.profile].nodes[node]
		{
			self.set_node_state(node, state);
		}
	}
}
