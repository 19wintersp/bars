use crate::{Brush,Pen};

pub struct Style {
	pub pen: Option<Pen>,
	pub brush: Option<Brush>,
}

impl Style {
	pub fn new(style: &bars_config::Style) -> Self {
		Self {
			pen: Pen::new(
				style.stroke_style,
				style.stroke_width,
				style.stroke_cap,
				style.stroke_join,
				style.stroke_color,
			),
			brush: Brush::new(style.fill_style, style.fill_color),
		}
	}
}

pub trait StyleSource {
	fn pen(&self) -> Option<&Pen> {
		None
	}

	fn brush(&self) -> Option<&Brush> {
		None
	}
}

impl StyleSource for Style {
	fn pen(&self) -> Option<&Pen> {
		self.pen.as_ref()
	}

	fn brush(&self) -> Option<&Brush> {
		self.brush.as_ref()
	}
}

impl StyleSource for Pen {
	fn pen(&self) -> Option<&Pen> {
		Some(self)
	}
}

impl StyleSource for Brush {
	fn brush(&self) -> Option<&Brush> {
		Some(self)
	}
}
