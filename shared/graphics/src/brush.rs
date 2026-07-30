use crate::convert_color;

use bars_config::{Color, FillStyle};

use windows::Win32::Graphics::GdiPlus::{GpBrush, HatchStyle};

#[repr(transparent)]
pub struct Brush(pub(crate) *mut GpBrush);

impl Brush {
	pub fn new(style: FillStyle, color: Color) -> Option<Self> {
		if style == FillStyle::None {
			None
		} else {
			Some(Self(if let FillStyle::Hatch(hatch) = style {
				let mut ptr = std::ptr::null_mut();
				c!(GdipCreateHatchBrush(
					HatchStyle(hatch),
					convert_color(color),
					0,
					&mut ptr,
				));
				ptr.cast()
			} else {
				let mut ptr = std::ptr::null_mut();
				c!(GdipCreateSolidFill(convert_color(color), &mut ptr));
				ptr.cast()
			}))
		}
	}
}

impl Drop for Brush {
	fn drop(&mut self) {
		c!(GdipDeleteBrush(self.0));
	}
}
