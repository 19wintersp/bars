use bars_config::{Geo, GeoPoint, MapPoint, Point, Projectable, Rect};
use bars_euroscope::{Position, RadarScreen};

#[derive(Debug)]
pub struct Transform {
	matrix: [f64; 6],
	scale: (f64, f64),
}

impl Transform {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_screen_geo(screen: &mut RadarScreen) -> Self {
		let origin = screen.unproject(bars_euroscope::Point { x: 0, y: 0 });
		let bounds = screen.bounds();

		let geo_min = bounds.bottom_left;
		let geo_max = bounds.top_right;
		let geo_lat = Position::new(geo_max.lat(), geo_min.lon());
		let geo_lon = Position::new(geo_min.lat(), geo_max.lon());

		let delta_lat = geo_max.lat() - geo_min.lat();
		let delta_lon = geo_max.lon() - geo_min.lon();

		let pos_min = screen.project(geo_min);
		let mut pos_lat = screen.project(geo_lat);
		let mut pos_lon = screen.project(geo_lon);

		pos_lat.x -= pos_min.x;
		pos_lat.y -= pos_min.y;
		pos_lon.x -= pos_min.x;
		pos_lon.y -= pos_min.y;

		let origin = (origin.lat(), origin.lon());
		let scale = (
			(pos_lat.x as f64).hypot(pos_lat.y as f64) / delta_lat,
			(pos_lon.x as f64).hypot(pos_lon.y as f64) / delta_lon,
		);
		let rotation = (pos_lon.x as f64).atan2(pos_lon.y as f64);

		let sin = rotation.sin();
		let cos = rotation.cos();

		let klat = -scale.0 * origin.0;
		let klon = -scale.1 * origin.1;

		Self {
			matrix: [
				cos,
				sin,
				klon * sin + klat * cos,
				-sin,
				cos,
				klon * cos - klat * sin,
			],
			scale,
		}
	}

	pub fn from_screen_map(screen: &mut RadarScreen, bounds: Rect) -> Self {
		let area = screen.radar_area();
		let size = (
			(area.right - area.left) as f64,
			(area.bottom - area.top) as f64,
		);

		let bounds_w = (bounds.max.x - bounds.min.x) as f64;
		let bounds_h = (bounds.max.y - bounds.min.y) as f64;

		let viewport_ratio = size.0 / size.1;
		let bounds_ratio = bounds_w / bounds_h;

		let (scale, offset_x, offset_y) = if bounds_ratio > viewport_ratio {
			let scale = size.0 / bounds_w;
			(scale, 0.0, (size.1 - bounds_h * scale) * 0.5)
		} else {
			let scale = size.1 / bounds_h;
			(scale, (size.0 - bounds_w * scale) * 0.5, 0.0)
		};

		Self {
			matrix: [
				1.0,
				0.0,
				scale * -bounds.min.x as f64 + offset_x,
				0.0,
				1.0,
				scale * -bounds.min.y as f64 + offset_y,
			],
			scale: (1.0, 1.0),
		}
	}

	#[inline(always)]
	fn transform(&self, (x, y): (f64, f64)) -> (f64, f64) {
		let (x, y) = self.transform_unscaled((x * self.scale.0, y * self.scale.1));
		(x + self.matrix[2], y + self.matrix[5])
	}

	#[inline(always)]
	fn transform_unscaled(&self, (x, y): (f64, f64)) -> (f64, f64) {
		(
			x * self.matrix[0] + y * self.matrix[1],
			x * self.matrix[3] + y * self.matrix[4],
		)
	}

	#[inline(always)]
	fn transform_geo(&self, geo: &Geo) -> (f64, f64) {
		self.transform((geo.lat as f64, geo.lon as f64))
	}

	#[inline(always)]
	fn transform_point(&self, point: &Point) -> (f64, f64) {
		self.transform((point.x as f64, point.y as f64))
	}

	#[inline(always)]
	fn transform_point_unscaled(&self, point: &Point) -> (f64, f64) {
		self.transform_unscaled((point.x as f64, point.y as f64))
	}

	#[inline(always)]
	fn transform_geo_point(&self, gp: &GeoPoint) -> (f64, f64) {
		let (x, y) = self.transform_geo(&gp.geo);
		let (vx, vy) = (gp.offset_view.x as f64, gp.offset_view.y as f64);
		let (gx, gy) = self.transform_point_unscaled(&gp.offset_grid);

		(x + vx + gx, y + vy + gy)
	}

	#[inline(always)]
	fn transform_map_point(&self, mp: &MapPoint) -> (f64, f64) {
		let (x, y) = self.transform_point(&mp.point);
		let (dx, dy) = (mp.offset.x as f64, mp.offset.y as f64);

		(x + dx, y + dy)
	}
}

impl Default for Transform {
	fn default() -> Self {
		Self {
			matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
			scale: (1.0, 1.0),
		}
	}
}

pub trait Transformable: Projectable {
	fn transform(&self, transform: &Transform) -> bars_graphics::Point;
}

impl Transformable for GeoPoint {
	fn transform(&self, transform: &Transform) -> bars_graphics::Point {
		to_graphics(transform.transform_geo_point(self))
	}
}

impl Transformable for MapPoint {
	fn transform(&self, transform: &Transform) -> bars_graphics::Point {
		to_graphics(transform.transform_map_point(self))
	}
}

fn to_graphics((x, y): (f64, f64)) -> bars_graphics::Point {
	bars_graphics::Point { x: x as f32, y: y as f32 }
}
