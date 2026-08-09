use windows::Win32::Graphics::GdiPlus::{PointF, RectF};

macro_rules! assert_struct_eq {
	( $s1:ident == $s2:ident { $( $f1:ident == $f2:ident ),* $(,)? } ) => {
		const _: () = const {
			use std::mem::{align_of, offset_of, size_of};

			[()][!(
				$(offset_of!($s1, $f1) == offset_of!($s2, $f2) &&)*
				size_of::<$s1>() == size_of::<$s2>() &&
				align_of::<$s1>() == align_of::<$s2>()
			) as usize];
		};
	};
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Point {
	pub x: f32,
	pub y: f32,
}

assert_struct_eq!(
	Point == PointF {
		x == X,
		y == Y,
	}
);

impl From<Point> for PointF {
	#[inline(always)]
	fn from(from: Point) -> PointF {
		unsafe { std::mem::transmute(from) }
	}
}

impl From<bars_config::Point> for Point {
	fn from(from: bars_config::Point) -> Point {
		Point {
			x: from.x,
			y: from.y,
		}
	}
}

impl From<Point> for bars_config::Point {
	fn from(from: Point) -> bars_config::Point {
		bars_config::Point {
			x: from.x,
			y: from.y,
		}
	}
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Rect {
	pub x: f32,
	pub y: f32,
	pub w: f32,
	pub h: f32,
}

assert_struct_eq!(
	Rect == RectF {
		x == X,
		y == Y,
		w == Width,
		h == Height,
	}
);

impl From<Rect> for RectF {
	#[inline(always)]
	fn from(from: Rect) -> RectF {
		unsafe { std::mem::transmute(from) }
	}
}

impl From<RectF> for Rect {
	#[inline(always)]
	fn from(from: RectF) -> Rect {
		unsafe { std::mem::transmute(from) }
	}
}
