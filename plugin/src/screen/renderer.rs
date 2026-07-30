use std::time::{Duration, Instant};

use super::lookup::Lookup2d;
use super::transform::{Transform, Transformable};
use super::{FONT_SIZE, GraphicsContext};
use crate::THEME_COLOR;
use crate::context::{Aerodrome, AerodromeMut};

use bars_config::{
	BlockState, CountdownStyle, FillStyle, Map, Node, Path, PathDisplay, Ref,
	State, StrokeCap, StrokeJoin, StrokeStyle, StrokeWidth, Target,
	TargetCommand, View, Widget,
};
use bars_euroscope::{MouseButton, MouseEvent, RadarScreen, SettingStore};
use bars_graphics::{Alignment, Brush, Pen, Point, Rect, StringFormat, Style};

const DESELECT_AFTER: Duration = Duration::from_secs(3);

static SETTING_KEY_VIEW: &str = "view";

pub struct Renderer {
	targets: Lookup2d<TargetCommand>,
	selection: Option<(Ref<Node>, Instant)>,
	view: Option<Ref<View>>,
	transform: Transform,
	styles: Vec<Style>,
}

impl Renderer {
	pub fn new(geo: bool) -> Self {
		Self {
			targets: Lookup2d::new(1, 1),
			selection: None,
			view: if geo { None } else { Some(0.into()) },
			transform: Transform::new(),
			styles: Vec::new(),
		}
	}

	pub fn set_view(&mut self, view: Ref<View>) {
		self.view = Some(view);
	}

	pub fn view(&self) -> Option<Ref<View>> {
		self.view
	}

	pub fn init(&mut self, ctx: &mut RadarScreen) {
		if let Some(view) = self
			.view
			.and_then(|_| ctx.load_setting::<usize>(SETTING_KEY_VIEW))
		{
			self.view = Some(view.into());
		}
	}

	fn find_map<'a>(&self, aerodrome: &'a Aerodrome) -> Option<&'a Map> {
		self.view.and_then(|view| {
			let mut i = view.0 as isize;
			aerodrome.config().maps.iter().find(|map| {
				i -= map.views.len() as isize;
				i < 0
			})
		})
	}

	fn project_points<T: Transformable>(&self, points: &[T]) -> Vec<Point> {
		points
			.iter()
			.map(|point| point.transform(&self.transform))
			.collect::<Vec<_>>()
	}

	fn render_message(
		&self,
		ctx: &mut RadarScreen,
		graphics: &GraphicsContext,
		message: &str,
	) {
		let area = ctx.radar_area();

		let origin = Point {
			x: 0.5 * (area.left + area.right) as f32,
			y: 0.5 * (area.top + area.bottom) as f32,
		};
		let format = StringFormat::new(Alignment::Center, Alignment::Center);

		graphics.graphics.draw_string(
			message,
			&graphics.font,
			origin,
			&format,
			&graphics.brush,
		);
	}

	pub fn render_backdrop(
		&mut self,
		ctx: &mut RadarScreen,
		aerodrome: &Aerodrome,
		graphics: &GraphicsContext,
	) {
		let area = ctx.radar_area();

		self.targets.reset(
			(area.right - area.left).max(1) as usize,
			(area.bottom - area.top).max(1) as usize,
		);
		self.styles = aerodrome
			.config()
			.styles
			.iter()
			.map(|style| Style::new(style))
			.collect();

		if let Some(view_ref) = self.view {
			let Some(view) = aerodrome
				.config()
				.maps
				.iter()
				.flat_map(|map| map.views.iter())
				.nth(view_ref.0)
			else {
				self.render_message(ctx, graphics, "No views defined for aerodrome");
				return
			};

			self.transform = Transform::from_screen_map(ctx, view.bounds);

			let map = self.find_map(aerodrome).unwrap();

			let brush = Brush::new(FillStyle::Fill, map.background).unwrap();
			graphics.graphics.draw_rectangle(
				Rect {
					x: area.left as f32,
					y: area.top as f32,
					w: (area.right - area.left) as f32,
					h: (area.bottom - area.top) as f32,
				},
				&brush,
			);

			self.render_backdrop_content(graphics, &map.paths, &map.targets);
		} else {
			self.transform = Transform::from_screen_geo(ctx);

			let Some(map) = &aerodrome.config().geo_map else {
				return
			};

			self.render_backdrop_content(graphics, &map.paths, &map.targets);
		}
	}

	fn render_backdrop_content<T: Transformable>(
		&mut self,
		graphics: &GraphicsContext,
		paths: &[Path<T>],
		targets: &[Target<T>],
	) {
		for path in paths {
			if let PathDisplay::Fixed { style } = path.display {
				let points = self.project_points(&path.points);
				graphics
					.graphics
					.draw_polygon(&points, &self.styles[style.0]);
			}
		}

		for target in targets {
			// todo: filter commands referencing fixed blocks/nodes

			for polygon in &target.polygons {
				let points = self.project_points(&polygon);
				self.targets.add_poly(target.command, &points);
			}
		}
	}

	pub fn render(
		&mut self,
		_ctx: &mut RadarScreen,
		aerodrome: &Aerodrome,
		graphics: &GraphicsContext,
	) {
		if self.view.is_some() {
			if let Some(map) = self.find_map(aerodrome) {
				self.render_content(aerodrome, graphics, &map.paths, &map.widgets);
			}
		} else {
			if let Some(map) = &aerodrome.config().geo_map {
				self.render_content(aerodrome, graphics, &map.paths, &map.widgets);
			}
		}
	}

	fn render_content<T: Transformable>(
		&mut self,
		aerodrome: &Aerodrome,
		graphics: &GraphicsContext,
		paths: &[Path<T>],
		widgets: &[Widget<T>],
	) {
		for path in paths {
			// this could be cached
			let points = self.project_points(&path.points);

			match path.display {
				PathDisplay::Fixed { .. } => (),
				PathDisplay::Node {
					node,
					off,
					on,
					selected,
				} => {
					match aerodrome.node_state(node).pending {
						State::Off => {
							if let Some(off) = off {
								graphics.graphics.draw_polygon(&points, &self.styles[off.0]);
							}
						},
						State::On => {
							if let Some(on) = on {
								graphics.graphics.draw_polygon(&points, &self.styles[on.0]);
							}
						},
					}

					if let Some(selected) = selected {
						if self.selection.is_some_and(|(selection, time)| {
							selection == node && time.elapsed() < DESELECT_AFTER
						}) {
							graphics
								.graphics
								.draw_polygon(&points, &self.styles[selected.0]);
						}
					}
				},
				PathDisplay::Bind {
					bind,
					off,
					on,
					pending,
				} => {
					let state = aerodrome.bind_state(bind);

					match state.current {
						State::Off => {
							if let Some(off) = off {
								graphics.graphics.draw_polygon(&points, &self.styles[off.0]);
							}
						},
						State::On => {
							if let Some(on) = on {
								graphics.graphics.draw_polygon(&points, &self.styles[on.0]);
							}
						},
					}

					if let Some(pending) = pending
						&& state.current != state.pending
					{
						graphics
							.graphics
							.draw_polygon(&points, &self.styles[pending.0]);
					}
				},
			}
		}

		for widget in widgets {
			match widget {
				Widget::Countdown {
					position,
					size: _,
					target,
					style: CountdownStyle::Generic,
				} => {
					if let Some(countdown) = aerodrome.countdown(target) {
						let centre = position.transform(&self.transform);
						let size = 1.5 * FONT_SIZE;
						let bbox = Rect {
							x: centre.x - 0.5 * size,
							y: centre.y - 0.5 * size,
							w: size,
							h: size,
						};

						let proportion = countdown.remaining().as_secs_f32()
							/ countdown.length.as_secs_f32();

						let pen = Pen::new(
							StrokeStyle::Dash(0),
							StrokeWidth::from(2.0),
							StrokeCap(0),
							StrokeJoin(2),
							THEME_COLOR,
						)
						.unwrap();
						graphics.graphics.draw_arc(
							bbox,
							90.0,
							-360.0 * (1.0 - proportion),
							&pen,
						);

						let format =
							StringFormat::new(Alignment::Center, Alignment::Center);
						graphics.graphics.draw_string(
							&countdown.remaining().as_secs().to_string(),
							&graphics.font,
							centre,
							&format,
							&graphics.brush,
						);
					}
				},
			}
		}
	}

	pub fn mouse_event(
		&mut self,
		_ctx: &mut RadarScreen,
		mut aerodrome: AerodromeMut<'_>,
		event: MouseEvent,
		position: bars_euroscope::Point,
	) {
		if position.x < 0 || position.y < 0 {
			return
		}

		let Some(command) = self
			.targets
			.sample(position.x as usize, position.y as usize)
		else {
			return
		};

		match event {
			MouseEvent::Click(MouseButton::Left) => match command {
				TargetCommand::Node(node) => {
					let selection = self.selection.take();

					if aerodrome.is_node_router(*node) {
						if let Some((selection, _)) =
							selection.filter(|(_, time)| time.elapsed() < DESELECT_AFTER)
						{
							aerodrome.insert_route(selection, *node);
						}

						self.selection = Some((*node, Instant::now()));
					} else {
						let state = match aerodrome.node_state(*node).pending {
							State::Off => State::On,
							State::On => State::Off,
						};
						aerodrome.set_node_state(*node, state);
					}
				},
				TargetCommand::Block(block) => {
					self.selection = None;
					aerodrome.set_block_state(*block, BlockState::Clear);
				},
				TargetCommand::Preset(preset) => aerodrome.apply_preset(*preset),
			},
			MouseEvent::Click(MouseButton::Right) => match command {
				TargetCommand::Block(block) => {
					aerodrome.set_block_state(*block, BlockState::Relax);
				},
				_ => (), // todo: scratchpad for nodes
			},
			_ => (),
		}
	}
}
