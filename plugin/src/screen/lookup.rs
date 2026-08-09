use bars_graphics::Point;

#[derive(Default)]
pub struct Lookup2d<T> {
	data: Vec<Option<T>>,
	width: usize,
}

impl<T: Copy> Lookup2d<T> {
	pub fn new(width: usize, height: usize) -> Self {
		Self {
			data: vec![None; width * height],
			width: width.max(1),
		}
	}

	pub fn reset(&mut self, width: usize, height: usize) {
		self.data.fill(None);
		self.data.resize(width * height, None);
		self.width = width.max(1);
	}

	pub fn sample(&self, x: usize, y: usize) -> Option<&T> {
		self
			.data
			.get((x + y * self.width).min(self.data.len() - 1))
			.map(|opt| opt.as_ref())
			.flatten()
	}

	pub fn add_poly(&mut self, item: T, points: &[Point]) {
		if points.is_empty() {
			return
		}

		let (min, max) = points
			.iter()
			.map(|point| point.y.max(0.0).round() as usize)
			.fold((usize::MAX, 0), |(min, max), y| (min.min(y), max.max(y)));
		let max_y = self.data.len() / self.width - 1;

		let min = min.min(max_y);
		let max = max.min(max_y);

		let mut intersections = Vec::new();
		for y in min..=max {
			let yf = y as f32 + 0.5;

			for i in 0..points.len() {
				let Point { x: x1, y: y1 } = points[i];
				let Point { x: x2, y: y2 } = points[(i + 1) % points.len()];

				if y1 != y2 && (y1 > yf) != (y2 > yf) {
					intersections.push(x1 + (x2 - x1) * (yf - y1) / (y2 - y1));
				}
			}

			intersections.sort_by(|a, b| a.partial_cmp(b).unwrap());

			for pair in intersections.chunks_exact(2) {
				let x1 = ((pair[0] - 0.5).round() as usize).min(self.width - 1);
				let x2 = ((pair[1] - 0.5).round() as usize).min(self.width - 1);

				self.data[y * self.width..][x1..=x2].fill(Some(item));
			}

			intersections.clear();
		}
	}
}
