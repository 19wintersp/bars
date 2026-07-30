use crate::WideString;

use windows::Win32::Graphics::GdiPlus::GpFontFamily;

#[repr(transparent)]
pub struct FontFamily(pub(crate) *mut GpFontFamily);

impl FontFamily {
	pub fn new(name: &str) -> Self {
		let name = WideString::new(name);
		let mut ptr = std::ptr::null_mut();
		c!(GdipCreateFontFamilyFromName(
			name.pcwstr(),
			std::ptr::null_mut(),
			&mut ptr
		));
		Self(ptr)
	}
}

impl Drop for FontFamily {
	fn drop(&mut self) {
		c!(GdipDeleteFontFamily(self.0));
	}
}
