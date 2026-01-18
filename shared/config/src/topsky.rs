use crate::*;

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub struct MapsLoadTopskyError {
	pub message: String,
	pub line: usize,
}

impl Display for MapsLoadTopskyError {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "line {}: {}", self.line, self.message)
	}
}

impl Error for MapsLoadTopskyError {}

enum Group {
	None,
	Base,
	Node(Ref<Node>, NodeGroup),
	Bind(Ref<Bind>, BindGroup),
	Block(Ref<Block>, BlockGroup),
	Preset(Ref<Preset>),
}

#[derive(PartialEq)]
enum NodeGroup {
	Off,
	On,
	Selected,
	Target,
}

#[derive(PartialEq)]
enum BindGroup {
	Off,
	On,
	Pending,
}

enum BlockGroup {
	Target,
}

struct Indexer<'a, T, U> {
	list: &'a mut Vec<T>,
	map: HashMap<U, usize>,
}

impl<'a, T, U> Indexer<'a, T, U> {
	fn new(list: &'a mut Vec<T>) -> Self {
		Self {
			list,
			map: HashMap::new(),
		}
	}
}

impl<'a, T: From<U>, U: Hash + Eq + Clone> Indexer<'a, T, U> {
	fn index<V>(&mut self, value: U) -> Ref<V> {
		let index = *self.map.entry(value.clone()).or_insert_with(|| {
			self.list.push(value.into());
			self.list.len() - 1
		});
		index.into()
	}
}

impl Maps {
	pub fn load_topsky(text: &str) -> Result<Self, MapsLoadTopskyError> {
		const DEFAULT_COLOR: Color = Color {
			r: 0,
			g: 0,
			b: 0,
			a: u8::MAX,
		};

		let mut maps = Self {
			nodes: Vec::new(),
			blocks: Vec::new(),
			binds: Vec::new(),
			presets: Vec::new(),

			geo_map: None,
			maps: Vec::new(),
			styles: Vec::new(),
		};

		let mut nodes = Indexer::new(&mut maps.nodes);
		let mut blocks = Indexer::new(&mut maps.blocks);
		let mut binds = Indexer::new(&mut maps.binds);
		let mut presets = Indexer::new(&mut maps.presets);
		let mut styles = Indexer::new(&mut maps.styles);

		let mut colors = HashMap::<String, Color>::new();

		let mut geo = None;
		let mut map = None;

		let mut coord_list = Vec::new();
		let mut point_list = Vec::new();

		let mut group = Group::None;

		let mut stroke_color = DEFAULT_COLOR;
		let mut stroke_style = StrokeStyle::None;
		let mut stroke_width = StrokeWidth::from(1.0);
		let mut fill_color = DEFAULT_COLOR;

		let lines = text
			.lines()
			.map(|line| {
				line
					.split_once("//")
					.map(|(line, _)| line)
					.unwrap_or(line)
					.trim()
			})
			.filter(|line| !line.is_empty())
			.map(|line| line.split(':').collect::<Vec<_>>())
			.enumerate()
			.map(|(i, line)| (i + 1, line));

		for (line, parts) in lines {
			let command = parts[0];
			let args = &parts[1..];

			macro_rules! bail {
				( $( $arg:tt )+ ) => {
					return Err(error!($($arg)+))
				};
			}

			macro_rules! error {
				( $( $arg:tt )+ ) => {
					MapsLoadTopskyError {
						message: format!($($arg)+),
						line,
					}
				}
			}

			macro_rules! check_args {
				( $expected:pat ) => {
					if !matches!(args.len(), $expected) {
						bail!(
							"incorrect number of arguments to {} (expected {}, got {})",
							command,
							stringify!($expected),
							args.len(),
						)
					}
				};
			}

			macro_rules! unwrap {
				( $result:expr ) => {
					$result.map_err(|err| error!("{err}"))?
				};
			}

			let parse_point = |parts: &[&str]| {
				Ok(Point {
					x: unwrap!(parts[0].parse::<f32>()),
					y: unwrap!(parts[1].parse::<f32>()),
				})
			};
			let parse_map_point = |parts: &[&str]| {
				Ok(MapPoint {
					point: parse_point(&parts)?,
					offset: if parts.len() > 2 {
						parse_point(&parts[2..])?
					} else {
						Point::default()
					},
				})
			};
			let parse_geo_point = |parts: &[&str]| {
				Ok(GeoPoint {
					geo: Geo {
						lat: unwrap!(parts[0].parse::<f32>()),
						lon: unwrap!(parts[1].parse::<f32>()),
					},
					offset_view: if parts.len() > 2 {
						parse_point(&parts[2..])?
					} else {
						Point::default()
					},
					offset_grid: if parts.len() > 4 {
						parse_point(&parts[4..])?
					} else {
						Point::default()
					},
				})
			};

			match command {
				"GEO" => {
					check_args!(0);

					if maps.geo_map.is_some() {
						bail!("geo map already defined")
					}

					map = None;
					maps.geo_map = Some(GeoMap::default());
					geo = maps.geo_map.as_mut();
				},
				"MAP" => {
					check_args!(0..=1);

					geo = None;
					maps.maps.push(Map {
						background: if let Some(color) = args.get(0) {
							*colors
								.get(*color)
								.ok_or_else(|| error!("{color} undefined"))?
						} else {
							DEFAULT_COLOR
						},
						..Map::default()
					});
					map = maps.maps.last_mut();
				},
				"VIEW" => {
					check_args!(5);

					if let Some(map) = &mut map {
						map.views.push(View {
							name: args[0].into(),
							bounds: Rect {
								min: parse_point(&args[1..3])?,
								max: parse_point(&args[3..5])?,
							},
						});
					} else {
						bail!("VIEW outside map context")
					}
				},
				"COLORDEF" => {
					check_args!(4);

					colors.insert(
						args[0].into(),
						Color {
							r: unwrap!(args[1].parse()),
							g: unwrap!(args[2].parse()),
							b: unwrap!(args[3].parse()),
							a: u8::MAX,
						},
					);
				},
				"COLOR" => {
					check_args!(1..=3);

					stroke_color = *colors
						.get(args[0])
						.ok_or_else(|| error!("{} undefined", args[0]))?;
					fill_color = *args
						.get(1)
						.map(|color| {
							colors
								.get(*color)
								.ok_or_else(|| error!("{color} undefined"))
						})
						.transpose()?
						.unwrap_or(&stroke_color);
				},
				"STYLE" => {
					check_args!(1..=2);

					stroke_style = match args[0].to_ascii_lowercase().as_str() {
						"null" => StrokeStyle::None,
						"solid" => StrokeStyle::Dash(0),
						"dash" => StrokeStyle::Dash(1),
						"dot" | "alternate" => StrokeStyle::Dash(2),
						"dashdot" => StrokeStyle::Dash(3),
						"dashdotdot" => StrokeStyle::Dash(4),
						other => bail!("unknown stroke style {other}"),
					};

					if let Some(width) = args.get(1) {
						stroke_width = unwrap!(width.parse::<f32>()).into();
						if stroke_width == 0f32.into() {
							stroke_style = StrokeStyle::None;
						}
					}
				},
				"NODE" => {
					check_args!(2);

					group = Group::Node(
						nodes.index(args[0]),
						match args[1] {
							"OFF" => NodeGroup::Off,
							"ON" => NodeGroup::On,
							"SELECTED" => NodeGroup::Selected,
							"TARGET" => NodeGroup::Target,
							other => bail!("unknown node group {other}"),
						},
					);
				},
				"BIND" => {
					check_args!(2);

					group = Group::Bind(
						binds.index(args[0]),
						match args[1] {
							"OFF" => BindGroup::Off,
							"ON" => BindGroup::On,
							"PENDING" => BindGroup::Pending,
							other => bail!("unknown bind group {other}"),
						},
					);
				},
				"BLOCK" => {
					check_args!(2);

					group = Group::Block(
						blocks.index(args[0]),
						match args[1] {
							"TARGET" => BlockGroup::Target,
							other => bail!("unknown block group {other}"),
						},
					);
				},
				"BASE" => {
					check_args!(0);

					group = Group::Base;
				},
				"PRESET" => {
					check_args!(1);

					group = Group::Preset(presets.index(args[0]));
				},
				"COORD" | "POINT" => {
					if geo.is_some() {
						check_args!(2 | 4 | 6);

						coord_list.push(parse_geo_point(&args)?);
					} else if map.is_some() {
						check_args!(2 | 4);

						point_list.push(parse_map_point(&args)?);
					} else {
						bail!("{command} outside map context")
					}
				},
				"COORDTARGET" | "POINTTARGET" => {
					check_args!(0);

					let target_command = match &group {
						Group::Node(i, NodeGroup::Target) => TargetCommand::Node(*i),
						Group::Block(i, BlockGroup::Target) => TargetCommand::Block(*i),
						Group::Preset(i) => TargetCommand::Preset(*i),
						_ => bail!("{command} outside target context"),
					};

					if let Some(geo) = &mut geo {
						geo.targets.push(Target {
							polygons: vec![std::mem::take(&mut coord_list)],
							command: target_command,
						});
					} else if let Some(map) = &mut map {
						map.targets.push(Target {
							polygons: vec![std::mem::take(&mut point_list)],
							command: target_command,
						});
					} else {
						bail!("{command} outside map context")
					}
				},
				"COORDLINE" | "COORDPOLY" | "POINTLINE" | "POINTPOLY" => {
					let fill_style = if let "COORDLINE" | "POINTLINE" = command {
						check_args!(0);

						FillStyle::None
					} else {
						check_args!(1);

						let fill = args[0];
						if fill.starts_with('E') {
							let n = unwrap!(fill[1..].parse::<i32>());
							if 0 <= n && n <= 52 {
								FillStyle::Hatch(n)
							} else {
								bail!("hatch enum variant out of range [0, 52]")
							}
						} else {
							match fill {
								"0" => FillStyle::None,
								"5" => FillStyle::Hatch(6),
								"10" => FillStyle::Hatch(7),
								"20" => FillStyle::Hatch(8),
								"25" => FillStyle::Hatch(9),
								"30" => FillStyle::Hatch(10),
								"40" => FillStyle::Hatch(11),
								"50" => FillStyle::Hatch(12),
								"60" => FillStyle::Hatch(13),
								"70" => FillStyle::Hatch(14),
								"75" => FillStyle::Hatch(15),
								"80" => FillStyle::Hatch(16),
								"90" => FillStyle::Hatch(17),
								"100" => FillStyle::Fill,
								_ => bail!("invalid hatch style {fill}"),
							}
						}
					};

					let style = Ref::from(styles.index(Style {
						stroke_style,
						stroke_width,
						stroke_cap: StrokeCap(0),
						stroke_join: StrokeJoin(0),
						stroke_color,
						fill_style,
						fill_color,
					}));

					let display = match &group {
						Group::Base => PathDisplay::Fixed { style },
						Group::Node(node, group) => PathDisplay::Node {
							node: *node,
							off: (group == &NodeGroup::Off).then_some(style),
							on: (group == &NodeGroup::On).then_some(style),
							selected: (group == &NodeGroup::Selected).then_some(style),
						},
						Group::Bind(bind, group) => PathDisplay::Bind {
							bind: *bind,
							off: (group == &BindGroup::Off).then_some(style),
							on: (group == &BindGroup::On).then_some(style),
							pending: (group == &BindGroup::Pending).then_some(style),
						},
						_ => bail!("{command} outside draw context"),
					};

					if let Some(geo) = &mut geo {
						geo.paths.push(Path {
							points: std::mem::take(&mut coord_list),
							display,
						});
					} else if let Some(map) = &mut map {
						map.paths.push(Path {
							points: std::mem::take(&mut point_list),
							display,
						});
					} else {
						bail!("{command} outside map context")
					}
				},
				"WIDGET" => {
					check_args!(1..);

					if geo.is_none() && map.is_none() {
						bail!("WIDGET outside map context")
					}

					match args[0] {
						"COUNTDOWN" => {
							check_args!(6..);

							let size = unwrap!(args[3].parse());
							let target = match args[1] {
								"NODE" => ResetTarget::Node(nodes.index(args[2])),
								"BLOCK" => ResetTarget::Block(blocks.index(args[2])),
								other => bail!("invalid counter condition {other}"),
							};

							if let Some(geo) = &mut geo {
								check_args!(6 | 8 | 10);

								geo.widgets.push(Widget::Countdown {
									position: parse_geo_point(&args[4..])?,
									size,
									target,
									style: CountdownStyle::Generic,
								});
							} else if let Some(map) = &mut map {
								check_args!(6);

								map.widgets.push(Widget::Countdown {
									position: parse_map_point(&args[4..])?,
									size,
									target,
									style: CountdownStyle::Generic,
								});
							}
						},
						other => bail!("unknown widget type {other}"),
					}
				},
				_ => bail!("unknown command {command}"),
			}
		}

		Ok(maps)
	}
}
