use crate::ffi;

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Position(ffi::CPosition);

impl Position {
	pub fn new(lat: f64, lon: f64) -> Self {
		Self(ffi::CPosition {
			m_Latitude: lat,
			m_Longitude: lon,
		})
	}

	pub fn lat(&self) -> f64 {
		self.0.m_Latitude
	}

	pub fn lon(&self) -> f64 {
		self.0.m_Longitude
	}

	pub fn set_lat(&mut self, lat: f64) {
		self.0.m_Latitude = lat;
	}

	pub fn set_lon(&mut self, lon: f64) {
		self.0.m_Longitude = lon;
	}
}

impl From<ffi::CPosition> for Position {
	fn from(from: ffi::CPosition) -> Self {
		Self(from)
	}
}

impl From<Position> for ffi::CPosition {
	fn from(from: Position) -> Self {
		from.0
	}
}
