use crate::ffi;

use std::ffi::CStr;

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct FlightPlan(ffi::CFlightPlan);

impl FlightPlan {
	pub(crate) fn new(inner: ffi::CFlightPlan) -> Option<Self> {
		(!inner.m_FpPosition.is_null()).then_some(Self(inner))
	}

	pub fn callsign(&self) -> &CStr {
		unsafe { CStr::from_ptr(self.0.GetCallsign()) }
	}
}

impl From<ffi::CFlightPlan> for Option<FlightPlan> {
	fn from(from: ffi::CFlightPlan) -> Self {
		FlightPlan::new(from)
	}
}

impl TryFrom<ffi::CFlightPlan> for FlightPlan {
	type Error = ();

	fn try_from(from: ffi::CFlightPlan) -> Result<Self, ()> {
		Self::new(from).ok_or(())
	}
}

impl From<FlightPlan> for ffi::CFlightPlan {
	fn from(from: FlightPlan) -> Self {
		from.0
	}
}
