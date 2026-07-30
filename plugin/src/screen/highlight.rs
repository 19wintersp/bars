use super::GraphicsContext;

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use bars_euroscope::Point;
use bars_graphics::Rect;

const MARK_RADIUS: f32 = 4.0;

#[derive(Clone, Copy, Default)]
pub enum Highlight {
	#[default]
	None,
	Mark,
}

impl Highlight {
	pub const ALL: [Self; 2] = [Self::None, Self::Mark];

	pub(super) fn render(&self, at: Point, graphics: &GraphicsContext) {
		match self {
			Self::None => (),
			Self::Mark => {
				graphics.graphics.draw_ellipse(
					Rect {
						x: at.x as f32 - MARK_RADIUS,
						y: at.y as f32 - MARK_RADIUS,
						w: 2.0 * MARK_RADIUS,
						h: 2.0 * MARK_RADIUS,
					},
					&graphics.brush,
				);
			},
		}
	}

	pub fn name(&self) -> &'static str {
		match self {
			Self::None => "None",
			Self::Mark => "Dot",
		}
	}
}

impl Display for Highlight {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::None => "none",
				Self::Mark => "mark",
			}
		)
	}
}

impl FromStr for Highlight {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"none" => Ok(Self::None),
			"mark" => Ok(Self::Mark),
			_ => Err(()),
		}
	}
}

impl From<Highlight> for usize {
	fn from(from: Highlight) -> Self {
		from as usize
	}
}

impl TryFrom<usize> for Highlight {
	type Error = ();

	fn try_from(from: usize) -> Result<Self, Self::Error> {
		[Highlight::None, Highlight::Mark]
			.get(from)
			.copied()
			.ok_or(())
	}
}
