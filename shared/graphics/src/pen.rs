use crate::convert_color;

use bars_config::{Color, StrokeCap, StrokeJoin, StrokeStyle, StrokeWidth};

use windows::Win32::Graphics::GdiPlus::{
	DashCap, DashStyle, GpPen, LineCap, LineJoin, UnitWorld,
};

#[repr(transparent)]
pub struct Pen(pub(crate) *mut GpPen);

impl Pen {
	pub fn new(
		style: StrokeStyle,
		width: StrokeWidth,
		cap: StrokeCap,
		join: StrokeJoin,
		color: Color,
	) -> Option<Self> {
		if let StrokeStyle::Dash(dash) = style {
			let mut ptr = std::ptr::null_mut();

			c!(GdipCreatePen1(
				convert_color(color),
				width.into(),
				UnitWorld,
				&mut ptr,
			));

			let cap = LineCap(cap.0);
			c!(GdipSetPenLineCap197819(ptr, cap, cap, DashCap(cap.0)));
			c!(GdipSetPenLineJoin(ptr, LineJoin(join.0)));
			c!(GdipSetPenDashStyle(ptr, DashStyle(dash)));

			Some(Self(ptr))
		} else {
			None
		}
	}
}

impl Drop for Pen {
	fn drop(&mut self) {
		c!(GdipDeletePen(self.0));
	}
}
