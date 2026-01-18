use crate::{Ref, BINCODE_CONFIG};

use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode};

/// A configuration defining an aerodrome.
#[derive(Clone, Debug, Decode, Encode)]
pub struct Aerodrome {
	pub nodes: Vec<Node>,
	pub blocks: Vec<Block>,
	pub binds: Vec<Bind>,

	pub profiles: Vec<Profile>,
	pub presets: Vec<Preset>,
}

impl Aerodrome {
	pub fn decode(serialised: &[u8]) -> Result<Self, DecodeError> {
		Ok(bincode::decode_from_slice(serialised, BINCODE_CONFIG)?.0)
	}

	pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
		bincode::encode_to_vec(self, BINCODE_CONFIG)
	}
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Node {
	pub id: String,

	pub scratchpad: Option<String>,
	pub children: usize,
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Block {
	pub id: String,

	pub nodes: Vec<Ref<Node>>,
	pub routes: Vec<BlockRoute>,

	pub stands: Vec<String>,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub struct BlockRoute {
	pub from: BlockNode,
	pub to: BlockNode,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub struct BlockNode {
	pub node: Ref<Node>,
	pub child: usize,
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Bind {
	pub id: String,

	pub elements: Vec<String>,
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Profile {
	pub id: String,
	pub name: String,

	pub nodes: Vec<NodeCondition>,
	pub blocks: Vec<BlockCondition>,
	pub binds: Vec<BindCondition>,
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum NodeCondition {
	Fixed {
		state: State,
	},
	Direct {
		reset: ResetCondition,
	},
	Router {
		/// A flag indicating that this node can not be automatically routed across.
		sticky: bool,
	},
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum BlockCondition {
	Fixed { state: BlockState },
	Router { reset: ResetCondition },
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum BindCondition {
	Fixed {
		state: State,
	},
	Bound {
		block: Option<Ref<Block>>,
		expression: BindExpression,
	},
}

/// An element in the boolean expression defining the status of a bind.
#[derive(Clone, Debug, Decode, Encode)]
pub enum BindDependency {
	/// A node reference.
	///
	/// Positive iff the node is in the **on** state.
	Node(Ref<Node>),
	/// A block route reference.
	///
	/// Positive iff this condition is associated with a block and the route is
	/// set in that block.
	Route(BlockRoute),
}

/// A boolean expression in disjunctive normal form.
#[derive(Clone, Debug, Decode, Encode)]
pub struct BindExpression {
	pub disjunction: Vec<BindConjunction>,
}

impl BindExpression {
	pub fn evaluate(&self, mut map: impl FnMut(&BindDependency) -> bool) -> bool {
		self
			.disjunction
			.iter()
			.any(|conjunction| conjunction.evaluate(&mut map))
	}
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct BindConjunction {
	pub positive: Vec<BindDependency>,
	pub negative: Vec<BindDependency>,
}

impl BindConjunction {
	pub fn evaluate(&self, mut map: impl FnMut(&BindDependency) -> bool) -> bool {
		self.positive.iter().all(|dep| map(dep))
			&& self.negative.iter().all(|dep| !map(dep))
	}
}

pub type ProfileFilter = Vec<Ref<Profile>>;

#[derive(Clone, Debug, Decode, Encode)]
pub struct ResetCondition {
	/// A timeout given in seconds; zero indicates no timeout.
	pub timeout: u8,
	/// The identifier for an element which, when crossed by an aircraft, triggers
	/// the reset.
	pub crossing: Option<String>,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum ResetTarget {
	Node(Ref<Node>),
	Block(Ref<Block>),
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
#[repr(u8)]
pub enum State {
	Off,
	On,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum BlockState {
	Clear,
	Relax,
	Route { from: Ref<Node>, to: Ref<Node> },
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Preset {
	pub id: String,
	/// A human-readable name for the preset.
	///
	/// If omitted, it will not be shown to the user.
	pub name: Option<String>,

	pub nodes: Vec<(Ref<Node>, State)>,
	pub blocks: Vec<(Ref<Block>, BlockState)>,

	pub profiles: ProfileFilter,
}
