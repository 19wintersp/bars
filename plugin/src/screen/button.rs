use super::{FONT_SIZE, GraphicsContext};

use bars_config::{Color, FillStyle};
use bars_euroscope::{Area, MouseEvent, Point, RadarScreen, SettingStore};
use bars_graphics::{Alignment, Brush, Rect, StringFormat};

const PADDING: f32 = FONT_SIZE / 4.0;

const BG_COLOR: Color = Color {
	r: 0x00,
	g: 0x00,
	b: 0x00,
	a: 0xff,
};
const FG_COLOR: Color = Color {
	r: 0xff,
	g: 0xff,
	b: 0xff,
	a: 0xff,
};

static SETTING_KEY_X: &str = "menuX";
static SETTING_KEY_Y: &str = "menuY";

pub struct Button {
	position: (f32, f32),
	area: Area,
	aerodrome: Option<String>,
	bg_brush: Brush,
	fg_brush: Brush,
	drag_offset: Option<(i32, i32)>,
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
			bg_brush: Brush::new(FillStyle::Fill, BG_COLOR).unwrap(),
			fg_brush: Brush::new(FillStyle::Fill, FG_COLOR).unwrap(),
			drag_offset: None,
		}
	}

	pub fn set_aerodrome(&mut self, aerodrome: &Option<String>) {
		self.aerodrome = aerodrome.clone();
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
		let size = (bbox.w + 2.0 * PADDING, FONT_SIZE + 2.0 * PADDING);

		let radar_area = ctx.radar_area();
		let area_size = (
			(radar_area.right - radar_area.left) as f32 - size.0 - 2.0 * PADDING,
			(radar_area.top - radar_area.bottom) as f32 - size.1 - 2.0 * PADDING,
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

		graphics.graphics.draw_rectangle(
			Rect {
				x: origin.0,
				y: origin.1,
				w: size.0,
				h: size.1,
			},
			&self.bg_brush,
		);
		graphics.graphics.draw_string(
			string,
			&graphics.font,
			bars_graphics::Point {
				x: origin.0 + PADDING,
				y: origin.1 + 0.5 * size.1,
			},
			&format,
			&self.fg_brush,
		);
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
			(radar_area.top - radar_area.bottom - size.1) as f32 - 2.0 * PADDING,
		);

		let offset = *self.drag_offset.get_or_insert_with(|| {
			(position.x - self.area.left, position.y - self.area.top)
		});
		let position = (
			(position.x - offset.0 - radar_area.left) as f32 - PADDING,
			(position.y - offset.1 - radar_area.top) as f32 - PADDING,
		);

		self.position = (position.0 / area_size.0, position.1 / area_size.1);

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
