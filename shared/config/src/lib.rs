mod map;

use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::io::{Error as IoError, Read, Write};
use std::marker::PhantomData;

use bincode::config::Configuration as BincodeConfig;
use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode};

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;

pub use map::*;

static MAGIC: &[u8] = b"\xffBARS\x13eu";

const BINCODE_CONFIG: BincodeConfig = bincode::config::standard();

pub trait Loadable: Decode<()> + Encode {
	const VERSION: u16;

	fn load(mut reader: impl Read) -> Result<Self, DecodeError> {
		fn bincode_error(error: IoError) -> DecodeError {
			DecodeError::Io {
				inner: error,
				additional: 0,
			}
		}

		let mut buf = vec![0; MAGIC.len()];
		reader.read_exact(&mut buf).map_err(bincode_error)?;

		if buf != MAGIC {
			return Err(DecodeError::Other("invalid config file"))
		}

		let mut buf = [0; 2];
		reader.read_exact(&mut buf).map_err(bincode_error)?;

		if buf != Self::VERSION.to_be_bytes() {
			return Err(DecodeError::Other("unsupported config version"))
		}

		let mut reader = DeflateDecoder::new(reader);
		bincode::decode_from_std_read(&mut reader, BINCODE_CONFIG)
	}

	fn save(&self, mut writer: impl Write) -> Result<(), EncodeError> {
		fn bincode_error(error: IoError) -> EncodeError {
			EncodeError::Io {
				inner: error,
				index: 0,
			}
		}

		writer.write_all(&MAGIC).map_err(bincode_error)?;
		writer
			.write_all(&Self::VERSION.to_be_bytes())
			.map_err(bincode_error)?;

		let mut writer = DeflateEncoder::new(writer, Compression::best());
		bincode::encode_into_std_write(self, &mut writer, BINCODE_CONFIG)?;

		Ok(())
	}
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Config {
	pub name: Option<String>,
	pub version: Option<String>,

	pub aerodromes: Vec<Aerodrome>,
}

impl Loadable for Config {
	const VERSION: u16 = 0x0001;
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Aerodrome {
	pub icao: String,

	pub elements: Vec<Element>,
	pub nodes: Vec<Node>,
	pub edges: Vec<Edge>,
	pub blocks: Vec<Block>,

	pub profiles: Vec<Profile>,

	pub geo_map: Option<GeoMap>,
	pub maps: Vec<Map>,
	pub styles: Vec<Style>,
}

impl Aerodrome {
	pub fn decode(serialised: &[u8]) -> Result<Self, DecodeError> {
		Ok(bincode::decode_from_slice(serialised, BINCODE_CONFIG)?.0)
	}

	pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
		bincode::encode_to_vec(self, BINCODE_CONFIG)
	}
}

#[derive(Debug, Decode, Encode)]
pub struct Ref<T>(pub usize, PhantomData<T>);

impl<T> Clone for Ref<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T> Copy for Ref<T> {}

impl<T> Hash for Ref<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.hash(state);
	}
}

impl<T> PartialEq for Ref<T> {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}

impl<T> Eq for Ref<T> {}

impl<T> PartialOrd for Ref<T> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.0.partial_cmp(&other.0)
	}
}

impl<T> Ord for Ref<T> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.0.cmp(&other.0)
	}
}

impl<T> From<usize> for Ref<T> {
	fn from(from: usize) -> Self {
		Self(from, PhantomData)
	}
}

impl<T> From<Ref<T>> for usize {
	fn from(from: Ref<T>) -> Self {
		from.0
	}
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Element {
	pub id: String,
	pub condition: ElementCondition,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum ElementCondition {
	Fixed(bool),
	Node(Ref<Node>),
	Edge(Ref<Edge>),
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Node {
	pub id: String,

	pub scratchpad: Option<String>,
	pub parent: Option<Ref<Node>>,
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Edge;

#[derive(Clone, Debug, Decode, Encode)]
pub struct Block {
	pub id: String,

	/// parent nodes only
	pub nodes: Vec<Ref<Node>>,
	pub edges: Vec<Ref<Edge>>,
	pub non_routes: Vec<BlockRoute>,

	pub stands: Vec<String>,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
/// child nodes only
pub struct BlockRoute {
	pub from: Ref<Node>,
	pub to: Ref<Node>,
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Profile {
	pub id: String,
	pub name: String,

	pub nodes: Vec<NodeCondition>,
	pub edges: Vec<EdgeCondition>,
	pub blocks: Vec<BlockCondition>,

	pub presets: Vec<Preset>,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum NodeCondition {
	Fixed { state: NodeState },
	Direct { reset: ResetCondition },
	Router { sticky: bool },
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum EdgeCondition {
	Fixed {
		state: EdgeState,
	},
	Direct {
		nodes: NodeExpression,
	},
	Router {
		block: Ref<Block>,
		routes: Vec<BlockRoute>,
	},
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct NodeExpression {
	pub disjunction: Vec<NodeConjunction>,
}

impl NodeExpression {
	pub fn evaluate(
		&self,
		node_state: &impl Fn(Ref<Node>) -> NodeState,
	) -> EdgeState {
		if self
			.disjunction
			.iter()
			.any(|conjunction| conjunction.evaluate(node_state))
		{
			EdgeState::On
		} else {
			EdgeState::Off
		}
	}
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct NodeConjunction {
	pub positive: Vec<Ref<Node>>,
	pub negative: Vec<Ref<Node>>,
}

impl NodeConjunction {
	fn evaluate(&self, node_state: &impl Fn(Ref<Node>) -> NodeState) -> bool {
		self
			.positive
			.iter()
			.all(|node| node_state(*node) == NodeState::On)
			&& self
				.negative
				.iter()
				.all(|node| node_state(*node) == NodeState::Off)
	}
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub struct BlockCondition {
	pub reset: ResetCondition,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum ResetCondition {
	None,
	TimeSecs(u32),
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Preset {
	pub name: String,

	pub nodes: Vec<(Ref<Node>, NodeState)>,
	pub blocks: Vec<(Ref<Block>, BlockState)>,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
#[repr(u8)]
pub enum NodeState {
	Off,
	On,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
#[repr(u8)]
pub enum EdgeState {
	Off,
	On,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum BlockState {
	Clear,
	Relax,
	/// parent nodes
	Route((Ref<Node>, Ref<Node>)),
}
