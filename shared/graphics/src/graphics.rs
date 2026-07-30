use crate::{
	Brush, Font, Pen, Point, Rect, StringFormat, StyleSource, WideString,
};

use std::ffi::c_void;

use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::Graphics::GdiPlus::{FillModeAlternate, GpGraphics, RectF};

pub struct Graphics {
	handle: HDC,
	graphics: *mut GpGraphics,
}

impl Graphics {
	pub fn new() -> Self {
		Self {
			handle: HDC(std::ptr::null_mut()),
			graphics: std::ptr::null_mut(),
		}
	}

	pub fn set_hdc(&mut self, hdc: *mut c_void) {
		let handle = HDC(hdc);

		if self.handle != handle {
			if !self.graphics.is_null() {
				c!(GdipDeleteGraphics(self.graphics));
			}

			self.handle = handle;
			c!(GdipCreateFromHDC(handle, &mut self.graphics));
		}
	}

	pub fn draw_polygon(&self, points: &[Point], style: &impl StyleSource) {
		let count = points.len() as i32;
		let points = points.as_ptr().cast();

		if let Some(Brush(brush)) = style.brush() {
			c!(GdipFillPolygon(
				self.graphics,
				*brush,
				points,
				count,
				FillModeAlternate,
			));
		}

		if let Some(Pen(pen)) = style.pen() {
			c!(GdipDrawLines(self.graphics, *pen, points, count));
		}
	}

	pub fn draw_rectangle(&self, bbox: Rect, style: &impl StyleSource) {
		if let Some(Brush(brush)) = style.brush() {
			c!(GdipFillRectangle(
				self.graphics,
				*brush,
				bbox.x,
				bbox.y,
				bbox.w,
				bbox.h,
			));
		}

		if let Some(Pen(pen)) = style.pen() {
			c!(GdipDrawRectangle(
				self.graphics,
				*pen,
				bbox.x,
				bbox.y,
				bbox.w,
				bbox.h,
			));
		}
	}

	pub fn draw_ellipse(&self, bbox: Rect, style: &impl StyleSource) {
		if let Some(Brush(brush)) = style.brush() {
			c!(GdipFillEllipse(
				self.graphics,
				*brush,
				bbox.x,
				bbox.y,
				bbox.w,
				bbox.h,
			));
		}

		if let Some(Pen(pen)) = style.pen() {
			c!(GdipDrawEllipse(
				self.graphics,
				*pen,
				bbox.x,
				bbox.y,
				bbox.w,
				bbox.h,
			));
		}
	}

	pub fn draw_arc(
		&self,
		bbox: Rect,
		start: f32,
		sweep: f32,
		style: &impl StyleSource,
	) {
		if let Some(Pen(pen)) = style.pen() {
			c!(GdipDrawArc(
				self.graphics,
				*pen,
				bbox.x,
				bbox.y,
				bbox.w,
				bbox.h,
				start,
				sweep,
			));
		}
	}

	pub fn draw_string(
		&self,
		string: &str,
		font: &Font,
		origin: Point,
		format: &StringFormat,
		brush: &Brush,
	) {
		let string = WideString::new(string);
		let rect = RectF {
			X: origin.x,
			Y: origin.y,
			Width: 0.0,
			Height: 0.0,
		};

		c!(GdipDrawString(
			self.graphics,
			string.pcwstr(),
			-1,
			font.0,
			&rect,
			format.0,
			brush.0,
		));
	}

	pub fn measure_string(
		&self,
		string: &str,
		font: &Font,
		origin: Point,
		format: &StringFormat,
	) -> Rect {
		let string = WideString::new(string);
		let rect = RectF {
			X: origin.x,
			Y: origin.y,
			Width: 0.0,
			Height: 0.0,
		};
		let mut bbox = RectF::default();

		c!(GdipMeasureString(
			self.graphics,
			string.pcwstr(),
			-1,
			font.0,
			&rect,
			format.0,
			&mut bbox,
			std::ptr::null_mut(),
			std::ptr::null_mut(),
		));

		bbox.into()
	}
}

impl Drop for Graphics {
	fn drop(&mut self) {
		if !self.graphics.is_null() {
			c!(GdipDeleteGraphics(self.graphics));
		}
	}
}
