use windows::Win32::Graphics::GdiPlus::{
	GpStringFormat, StringAlignment, StringAlignmentCenter, StringAlignmentFar,
	StringAlignmentNear,
};

#[repr(transparent)]
pub struct StringFormat(pub(crate) *mut GpStringFormat);

impl StringFormat {
	pub fn new(horizontal: Alignment, vertical: Alignment) -> Self {
		let mut ptr = std::ptr::null_mut();
		c!(GdipStringFormatGetGenericDefault(&mut ptr));
		c!(GdipSetStringFormatAlign(
			ptr,
			StringAlignment(horizontal as i32)
		));
		c!(GdipSetStringFormatLineAlign(
			ptr,
			StringAlignment(vertical as i32)
		));
		Self(ptr)
	}
}

impl Drop for StringFormat {
	fn drop(&mut self) {
		c!(GdipDeleteStringFormat(self.0));
	}
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum Alignment {
	#[default]
	Near = StringAlignmentNear.0,
	Center = StringAlignmentCenter.0,
	Far = StringAlignmentFar.0,
}
