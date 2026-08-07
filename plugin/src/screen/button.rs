use super::{FONT_SIZE, GraphicsContext};

use bars_config::{
	Color, FillStyle, Icao, StrokeCap, StrokeJoin, StrokeStyle, StrokeWidth,
};
use bars_euroscope::{Area, MouseEvent, Point, RadarScreen, SettingStore};
use bars_graphics::{Alignment, Brush, Pen, Rect, StringFormat};
use bars_ipc::{AerodromeState, ConnectionTarget};

const PADDING: f32 = 0.2 * FONT_SIZE;
const GLYPH_SIZE: f32 = 0.5 * FONT_SIZE;

const WHITE: Color = color(0xff, 0xff, 0xff);

static DISCONNECTED_GLYPH: &[(f32, f32)] =
	&[(0.0, 0.0), (1.0, 1.0), (0.5, 0.5), (0.0, 1.0), (1.0, 0.0)];
static CONNECTED_GLYPH: &[(f32, f32)] = &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)];
static LOCAL_GLYPH: &[(f32, f32)] = &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)];

static SETTING_KEY_X: &str = "menuX";
static SETTING_KEY_Y: &str = "menuY";

const fn color(r: u8, g: u8, b: u8) -> Color {
	Color { r, g, b, a: 0xff }
}

pub struct Button {
	position: (f32, f32),
	area: Area,
	aerodrome: Option<Icao>,
	network: ConnectionTarget,
	capacity: AerodromeState,
	drag_offset: Option<(i32, i32)>,
	white_brush: Brush,
	white_pen: Pen,
}

impl Button {
	pub fn new() -> Self {
		Button {
			position: Default::default(),
			area: Area {
				left: 0,
				top: 0,
				right: 0,
				bottom: 0,
			},
			aerodrome: None,
			network: ConnectionTarget::None,
			capacity: AerodromeState::None,
			drag_offset: None,
			white_brush: Brush::new(FillStyle::Fill, WHITE).unwrap(),
			white_pen: Pen::new(
				StrokeStyle::Dash(0),
				StrokeWidth::from(1.0),
				StrokeCap(0),
				StrokeJoin(0),
				WHITE,
			)
			.unwrap(),
		}
	}

	pub fn set_aerodrome(&mut self, aerodrome: Option<Icao>) {
		self.aerodrome = aerodrome;
	}

	pub fn set_state(
		&mut self,
		network: ConnectionTarget,
		capacity: AerodromeState,
	) {
		self.network = network;
		self.capacity = capacity;
	}

	pub fn init(&mut self, ctx: &mut RadarScreen) {
		self.position = (
			ctx.load_setting::<f32>(SETTING_KEY_X).unwrap_or(1.0),
			ctx.load_setting::<f32>(SETTING_KEY_Y).unwrap_or(0.0),
		);
	}

	pub fn area(&self) -> Area {
		self.area
	}

	pub fn render(&mut self, ctx: &mut RadarScreen, graphics: &GraphicsContext) {
		let string = self
			.aerodrome
			.as_ref()
			.map(|s| s.as_str())
			.unwrap_or("BARS");
		let format = StringFormat::new(Alignment::Near, Alignment::Center);

		let bbox = graphics.graphics.measure_string(
			string,
			&graphics.font,
			bars_graphics::Point { x: 0.0, y: 0.0 },
			&format,
		);
		let size = (
			bbox.w + FONT_SIZE + 3.0 * PADDING,
			FONT_SIZE + 2.0 * PADDING,
		);

		let radar_area = ctx.radar_area();
		let area_size = (
			(radar_area.right - radar_area.left) as f32 - size.0 - 2.0 * PADDING,
			(radar_area.bottom - radar_area.top) as f32 - size.1 - 2.0 * PADDING,
		);

		let origin = (
			radar_area.left as f32 + PADDING + self.position.0 * area_size.0,
			radar_area.top as f32 + PADDING + self.position.1 * area_size.1,
		);

		self.area = Area {
			left: origin.0 as i32,
			top: origin.1 as i32,
			right: (origin.0 + size.0) as i32,
			bottom: (origin.1 + size.1) as i32,
		};

		let bg_brush = Brush::new(
			FillStyle::Fill,
			match self.capacity {
				AerodromeState::None => color(0x26, 0x26, 0x26),
				AerodromeState::Loading => color(0x4c, 0x4c, 0x4c),
				AerodromeState::Error => color(0x99, 0x1b, 0x1b),
				AerodromeState::Observe => color(0x37, 0x30, 0xa3),
				AerodromeState::Control => color(0x06, 0x4e, 0x3b),
			},
		)
		.unwrap();

		let glyph = match self.network {
			ConnectionTarget::None => DISCONNECTED_GLYPH,
			ConnectionTarget::Network => CONNECTED_GLYPH,
			ConnectionTarget::Local => LOCAL_GLYPH,
		}
		.iter()
		.map(|(rx, ry)| bars_graphics::Point {
			x: origin.0 + PADDING + GLYPH_SIZE * rx + 0.5 * (FONT_SIZE - GLYPH_SIZE),
			y: origin.1 + PADDING + GLYPH_SIZE * ry + 0.5 * (FONT_SIZE - GLYPH_SIZE),
		})
		.collect::<Vec<_>>();

		graphics.graphics.draw_rectangle(
			Rect {
				x: origin.0,
				y: origin.1,
				w: size.0,
				h: size.1,
			},
			&bg_brush,
		);
		graphics.graphics.draw_string(
			string,
			&graphics.font,
			bars_graphics::Point {
				x: origin.0 + FONT_SIZE + 2.0 * PADDING,
				y: origin.1 + 0.5 * size.1,
			},
			&format,
			&self.white_brush,
		);
		graphics.graphics.draw_polygon(&glyph, &self.white_pen);
	}

	pub fn mouse_event(
		&mut self,
		ctx: &mut RadarScreen,
		event: MouseEvent,
		position: Point,
	) {
		let size = (
			self.area.right - self.area.left,
			self.area.bottom - self.area.top,
		);

		let radar_area = ctx.radar_area();
		let area_size = (
			(radar_area.right - radar_area.left - size.0) as f32 - 2.0 * PADDING,
			(radar_area.bottom - radar_area.top - size.1) as f32 - 2.0 * PADDING,
		);

		let offset = *self.drag_offset.get_or_insert_with(|| {
			(position.x - self.area.left, position.y - self.area.top)
		});
		let position = (
			(position.x - offset.0 - radar_area.left) as f32 - PADDING,
			(position.y - offset.1 - radar_area.top) as f32 - PADDING,
		);

		self.position = (
			(position.0 / area_size.0).clamp(0.0, 1.0),
			(position.1 / area_size.1).clamp(0.0, 1.0),
		);

		if event == MouseEvent::DragEnd {
			self.drag_offset = None;

			ctx.save_setting(
				SETTING_KEY_X,
				&self.position.0,
				c"Menu button X position",
			);
			ctx.save_setting(
				SETTING_KEY_Y,
				&self.position.1,
				c"Menu button Y position",
			);
		}
	}
}
