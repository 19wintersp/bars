use std::collections::HashMap;
use std::sync::Arc;

use bars_graph::{Graph as GraphImpl, MapUpdate, Patch};
use bars_ipc::{AerodromeConfig, GraphAction};

use ouroboros::self_referencing;

#[self_referencing]
pub struct Graph {
	config: Arc<AerodromeConfig>,
	#[borrows(config)]
	#[covariant]
	graph: GraphImpl<'this>,
}

impl Graph {
	pub fn new_with_patch(config: Arc<AerodromeConfig>, patch: Patch) -> Self {
		Self::new(config, |config| {
			let mut graph = GraphImpl::new(&config.config, rand::random());
			if !patch.is_valid_initial() {
				graph.set_profile(0.into());
			}
			graph.apply_patch(&patch);
			graph
		})
	}

	pub fn update_crossing(&mut self, object: &str) {
		self.with_graph_mut(|graph| graph.update_crossing(object));
	}

	pub fn apply_action(&mut self, action: GraphAction) {
		self.with_graph_mut(|graph| match action {
			GraphAction::SetProfile(profile) => graph.set_profile(profile),
			GraphAction::ApplyPreset(preset) => graph.apply_preset(preset),
			GraphAction::SetNodeState(node, state) => {
				graph.set_node_state(node, state)
			},
			GraphAction::SetBlockState(block, state) => {
				graph.set_block_state(block, state)
			},
			GraphAction::InsertRoute(from, to) => graph.insert_route(from, to),
		});
	}

	pub fn apply_patch(&mut self, patch: &Patch) {
		self.with_graph_mut(|graph| graph.apply_patch(patch));
	}

	pub fn tick(&mut self) -> (Option<Patch>, HashMap<String, bool>) {
		self.with_graph_mut(|graph| {
			graph.update_timers();
			(graph.take_patch(), graph.take_element_updates())
		})
	}

	pub fn take_map_updates(&mut self) -> Vec<MapUpdate> {
		self.with_graph_mut(|graph| graph.take_map_updates())
	}

	pub fn initial_map_updates(&self) -> Vec<MapUpdate> {
		self.with_graph(|graph| graph.initial_map_updates())
	}
}
