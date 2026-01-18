use crate::*;

#[derive(Clone, Debug, Decode, Encode)]
pub struct Maps {
	pub nodes: Vec<String>,
	pub blocks: Vec<String>,
	pub binds: Vec<String>,
	pub presets: Vec<String>,

	pub geo_map: Option<GeoMap>,
	pub maps: Vec<Map>,
	pub styles: Vec<Style>,
}

impl Loadable for Maps {
	const VERSION: u16 = 0x8003;
}

#[derive(Clone, Debug, Default, Decode, Encode)]
pub struct GeoMap {
	pub paths: Vec<Path<GeoPoint>>,
	pub targets: Vec<Target<GeoPoint>>,
	pub widgets: Vec<Widget<GeoPoint>>,
}

#[derive(Clone, Debug, Default, Decode, Encode)]
pub struct Map {
	pub background: Color,
	pub paths: Vec<Path<MapPoint>>,
	pub targets: Vec<Target<MapPoint>>,
	pub widgets: Vec<Widget<MapPoint>>,
	pub views: Vec<View>,
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct View {
	pub name: String,
	pub bounds: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Decode, Encode)]
pub struct Rect {
	pub min: Point,
	pub max: Point,
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Path<T: Projectable> {
	pub points: Vec<T>,
	pub display: PathDisplay,
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum PathDisplay {
	Fixed {
		style: Ref<Style>,
	},
	Node {
		node: Ref<Node>,
		off: Option<Ref<Style>>,
		on: Option<Ref<Style>>,
		selected: Option<Ref<Style>>,
	},
	Bind {
		bind: Ref<Bind>,
		off: Option<Ref<Style>>,
		on: Option<Ref<Style>>,
		pending: Option<Ref<Style>>,
	},
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct Target<T: Projectable> {
	pub polygons: Vec<Vec<T>>,
	pub command: TargetCommand,
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum TargetCommand {
	Node(Ref<Node>),
	Block(Ref<Block>),
	Preset(Ref<Preset>),
}

#[derive(Clone, Debug, Decode, Encode)]
pub enum Widget<T: Projectable> {
	Countdown {
		position: T,
		size: f32,
		target: ResetTarget,
		style: CountdownStyle,
	},
}

#[derive(
	Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Decode, Encode,
)]
pub enum CountdownStyle {
	Generic,
}

mod sealed {
	use std::fmt::Debug;

	pub trait Sealed {}
	pub trait Projectable: Clone + Debug + Sealed {}
}

use sealed::Sealed;
pub use sealed::Projectable;

#[derive(
	Clone, Copy, Debug, Default, PartialEq, PartialOrd, Decode, Encode,
)]
pub struct Point {
	pub x: f32,
	pub y: f32,
}

impl Sealed for Point {}
impl Projectable for Point {}

#[derive(
	Clone, Copy, Debug, Default, PartialEq, PartialOrd, Decode, Encode,
)]
pub struct MapPoint {
	pub point: Point,
	/// An offset in screen-space pixels applied in line with the screen axes but
	/// without the scaling applied to the main point.
	pub offset: Point,
}

impl Sealed for MapPoint {}
impl Projectable for MapPoint {}

#[derive(
	Clone, Copy, Debug, Default, PartialEq, PartialOrd, Decode, Encode,
)]
pub struct Geo {
	pub lat: f32,
	pub lon: f32,
}

impl Sealed for Geo {}
impl Projectable for Geo {}

#[derive(
	Clone, Copy, Debug, Default, PartialEq, PartialOrd, Decode, Encode,
)]
pub struct GeoPoint {
	pub geo: Geo,
	/// An offset in screen-space pixels applied in line with the screen axes.
	pub offset_view: Point,
	/// An offset in screen-space pixels applied in line with the geographic axes.
	pub offset_grid: Point,
}

impl Sealed for GeoPoint {}
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
