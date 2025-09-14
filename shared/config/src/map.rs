use super::*;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
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

#[derive(Clone, Debug, Decode, Encode)]
pub struct GeoMap {
	pub nodes: Vec<NodeDisplay<GeoPoint>>,
	pub edges: Vec<EdgeDisplay<GeoPoint>>,
	pub blocks: Vec<BlockDisplay<GeoPoint>>,
	pub widgets: Vec<Widget<GeoPoint>>,
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

#[derive(Clone, Debug, Default, Decode, Encode)]
pub struct EdgeDisplay<T: Projectable> {
	pub off: Vec<Path<T>>,
	pub on: Vec<Path<T>>,
	pub pending: Vec<Path<T>>,
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
