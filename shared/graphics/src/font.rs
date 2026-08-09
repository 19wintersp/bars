use crate::FontFamily;

use windows::Win32::Graphics::GdiPlus::{FontStyleRegular, GpFont, UnitPixel};

#[repr(transparent)]
pub struct Font(pub(crate) *mut GpFont);

impl Font {
	pub fn new(family: &FontFamily, em_size: f32) -> Self {
		let mut ptr = std::ptr::null_mut();
		c!(GdipCreateFont(
			family.0,
			em_size,
			FontStyleRegular.0,
			UnitPixel,
			&mut ptr
		));
		Self(ptr)
	}
}

impl Drop for Font {
	fn drop(&mut self) {
		c!(GdipDeleteFont(self.0));
	}
}
