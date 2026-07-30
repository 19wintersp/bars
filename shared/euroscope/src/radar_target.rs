use crate::{Position, ffi};

use std::ffi::CStr;

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct RadarTarget(ffi::CRadarTarget);

impl RadarTarget {
	pub(crate) fn new(inner: ffi::CRadarTarget) -> Option<Self> {
		(!inner.m_RtPosition.is_null()).then_some(Self(inner))
	}

	pub fn callsign(&self) -> &CStr {
		unsafe { CStr::from_ptr(self.0.GetCallsign()) }
	}

	pub fn position(&self) -> Position {
		unsafe { ffi::RadarTarget_GetPosition(&self.0).into() }
	}
}

impl From<ffi::CRadarTarget> for Option<RadarTarget> {
	fn from(from: ffi::CRadarTarget) -> Self {
		RadarTarget::new(from)
	}
}

impl TryFrom<ffi::CRadarTarget> for RadarTarget {
	type Error = ();

	fn try_from(from: ffi::CRadarTarget) -> Result<Self, ()> {
		Self::new(from).ok_or(())
	}
}

impl From<RadarTarget> for ffi::CRadarTarget {
	fn from(from: RadarTarget) -> Self {
		from.0
	}
}
