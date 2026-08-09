macro_rules! c {
	($call:ident($($arg:expr),*$(,)?)) => {{
		use tracing::warn;
		use windows::Win32::Graphics::GdiPlus;

		let status = unsafe { GdiPlus::$call($($arg),*) };
		if status.0 != 0 {
			warn!(
				"{} returned {status:?}",
				stringify!($call),
			);
		}
	}};
}

mod brush;
mod font;
mod font_family;
mod graphics;
mod pen;
mod primitive;
mod string_format;
mod style;

pub use brush::Brush;
pub use font::Font;
pub use font_family::FontFamily;
pub use graphics::Graphics;
pub use pen::Pen;
pub use primitive::{Point, Rect};
pub use string_format::{Alignment, StringFormat};
pub use style::{Style, StyleSource};

use windows::Win32::Graphics::GdiPlus::{GdiplusShutdown, GdiplusStartupInput};
use windows::core::PCWSTR;

static mut TOKEN: usize = usize::MAX;

pub fn startup() {
	let input = GdiplusStartupInput {
		GdiplusVersion: 1,
		..Default::default()
	};
	c!(GdiplusStartup(&raw mut TOKEN, &input, std::ptr::null_mut()));
}

pub fn shutdown() {
	unsafe {
		GdiplusShutdown(TOKEN);
	}
}

fn convert_color(color: bars_config::Color) -> u32 {
	u32::from_be_bytes([color.a, color.r, color.g, color.b])
}

struct WideString(Vec<u16>);

impl WideString {
	fn new(string: &str) -> Self {
		Self(string.encode_utf16().chain([0]).collect())
	}

	fn pcwstr(&self) -> PCWSTR {
		PCWSTR(self.0.as_ptr())
	}
}
