use super::*;

#[derive(Clone, Debug, Decode, Encode)]
pub struct Maps {
	pub nodes: Vec<String>,
	pub edges: Vec<String>,
	pub blocks: Vec<String>,

	pub geo_map: Option<GeoMap>,
	pub maps: Vec<Map>,
	pub styles: Vec<Style>,
}

impl Loadable for Maps {
	const VERSION: u16 = 0x8002;
}

pub(crate) struct Rebase {
	pub offset: usize,
	pub nodes: Vec<Option<usize>>,
	pub edges: Vec<Option<usize>>,
	pub blocks: Vec<Option<usize>>,
}

fn rebase_vec<T: Default>(
	mut source: Vec<T>,
	rebase: &[Option<usize>],
	offset: impl Fn(&mut T),
) -> Vec<T> {
	rebase
		.iter()
		.map(|i| {
			i.map(|i| std::mem::take(&mut source[i]))
				.unwrap_or_default()
		})
		.map(|mut t| {
			offset(&mut t);
			t
		})
		.collect()
}

fn offset_paths<T: Projectable>(paths: &mut [Path<T>], offset: usize) {
	paths.iter_mut().for_each(|path| path.style.0 += offset);
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct GeoMap {
	pub nodes: Vec<NodeDisplay<GeoPoint>>,
	pub edges: Vec<EdgeDisplay<GeoPoint>>,
	pub blocks: Vec<BlockDisplay<GeoPoint>>,
	pub widgets: Vec<Widget<GeoPoint>>,
}

impl GeoMap {
	pub(crate) fn rebase(self, rebase: &Rebase) -> Self {
		Self {
			nodes: rebase_vec(self.nodes, &rebase.nodes, |d| d.offset(rebase.offset)),
			edges: rebase_vec(self.edges, &rebase.edges, |d| d.offset(rebase.offset)),
			blocks: rebase_vec(self.blocks, &rebase.blocks, |_| ()),
			widgets: self
				.widgets
				.into_iter()
				.filter_map(|w| w.rebase(rebase))
				.collect(),
		}
	}
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Map {
	pub background: Color,
	pub base: Vec<Path<Point>>,

	pub nodes: Vec<NodeDisplay<Point>>,
	pub edges: Vec<EdgeDisplay<Point>>,
	pub blocks: Vec<BlockDisplay<Point>>,
	pub widgets: Vec<Widget<Point>>,

	pub views: Vec<View>,
}

impl Map {
	pub(crate) fn rebase(mut self, rebase: &Rebase) -> Self {
		offset_paths(&mut self.base, rebase.offset);
		Self {
			nodes: rebase_vec(self.nodes, &rebase.nodes, |d| d.offset(rebase.offset)),
			edges: rebase_vec(self.edges, &rebase.edges, |d| d.offset(rebase.offset)),
			blocks: rebase_vec(self.blocks, &rebase.blocks, |_| ()),
			widgets: self
				.widgets
				.into_iter()
				.filter_map(|w| w.rebase(rebase))
				.collect(),
			..self
		}
	}
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct View {
	pub name: String,
	pub bounds: Box,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Decode, Encode)]
pub struct Box {
	pub min: Point,
	pub max: Point,
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Path<T: Projectable> {
	pub points: Vec<T>,
	pub style: Ref<Style>,
}

#[derive(Clone, Debug, Default, Decode, Encode)]
pub struct Target<T: Projectable> {
	pub polygons: Vec<Vec<T>>,
}

#[derive(Clone, Debug, Default, Decode, Encode)]
pub struct NodeDisplay<T: Projectable> {
	pub off: Vec<Path<T>>,
	pub on: Vec<Path<T>>,
	pub selected: Vec<Path<T>>,

	pub target: Target<T>,
}

impl<T: Projectable> NodeDisplay<T> {
	fn offset(&mut self, offset: usize) {
		offset_paths(&mut self.off, offset);
		offset_paths(&mut self.on, offset);
		offset_paths(&mut self.selected, offset);
	}
}

#[derive(Clone, Debug, Default, Decode, Encode)]
pub struct EdgeDisplay<T: Projectable> {
	pub off: Vec<Path<T>>,
	pub on: Vec<Path<T>>,
	pub pending: Vec<Path<T>>,
}

impl<T: Projectable> EdgeDisplay<T> {
	fn offset(&mut self, offset: usize) {
		offset_paths(&mut self.off, offset);
		offset_paths(&mut self.on, offset);
		offset_paths(&mut self.pending, offset);
	}
}

#[derive(Clone, Debug, Default, Decode, Encode)]
pub struct BlockDisplay<T: Projectable> {
	pub target: Target<T>,
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum Widget<T: Projectable> {
	Countdown {
		position: T,
		size: f32,
		condition: CountdownCondition,
	},
}

impl<T: Projectable> Widget<T> {
	fn rebase(mut self, rebase: &Rebase) -> Option<Self> {
		let Self::Countdown { condition, .. } = &mut self;
		match condition {
			CountdownCondition::Node(i) => {
				*i = rebase.nodes.iter().position(|j| *j == Some(i.0))?.into();
			},
			CountdownCondition::Block(i) => {
				*i = rebase.blocks.iter().position(|j| *j == Some(i.0))?.into();
			},
		}

		Some(self)
	}
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum CountdownCondition {
	Node(Ref<Node>),
	Block(Ref<Block>),
}

pub trait Projectable: Clone + Debug {}

#[derive(
	Clone, Copy, Debug, Default, PartialEq, PartialOrd, Decode, Encode,
)]
pub struct Point {
	pub x: f32,
	pub y: f32,
}

impl Projectable for Point {}

#[derive(
	Clone, Copy, Debug, Default, PartialEq, PartialOrd, Decode, Encode,
)]
pub struct Geo {
	pub lat: f32,
	pub lon: f32,
}

impl Projectable for Geo {}

#[derive(
	Clone, Copy, Debug, Default, PartialEq, PartialOrd, Decode, Encode,
)]
pub struct GeoPoint {
	pub geo: Geo,
	pub offset: Point,
}

impl Projectable for GeoPoint {}

#[derive(
	Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub struct Style {
	pub stroke_style: StrokeStyle,
	pub stroke_width: StrokeWidth,
	pub stroke_cap: StrokeCap,
	pub stroke_join: StrokeJoin,
	pub stroke_color: Color,

	pub fill_style: FillStyle,
	pub fill_color: Color,
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub struct Color {
	pub r: u8,
	pub g: u8,
	pub b: u8,
	pub a: u8,
}

impl Default for Color {
	fn default() -> Self {
		Self {
			r: 0xff,
			g: 0x00,
			b: 0xff,
			a: 0x00,
		}
	}
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum StrokeStyle {
	None,
	Dash(i32),
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub struct StrokeWidth(u8);

impl From<StrokeWidth> for f32 {
	fn from(from: StrokeWidth) -> Self {
		from.0 as f32 / 8.0
	}
}

impl From<f32> for StrokeWidth {
	fn from(from: f32) -> Self {
		Self((8.0 * from).clamp(0.0, 255.0).round() as u8)
	}
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub struct StrokeCap(pub i32);

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub struct StrokeJoin(pub i32);

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum FillStyle {
	None,
	Fill,
	Hatch(i32),
}
