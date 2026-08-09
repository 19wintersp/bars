use crate::ffi;

use std::ffi::CStr;

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Controller(ffi::CController);

impl Controller {
	pub(crate) fn new(inner: ffi::CController) -> Option<Self> {
		(!inner.m_CtrPosition.is_null() || inner.m_Myself).then_some(Self(inner))
	}

	pub fn callsign(&self) -> &CStr {
		unsafe { CStr::from_ptr(self.0.GetCallsign()) }
	}
}

impl From<ffi::CController> for Option<Controller> {
	fn from(from: ffi::CController) -> Self {
		Controller::new(from)
	}
}

impl TryFrom<ffi::CController> for Controller {
	type Error = ();

	fn try_from(from: ffi::CController) -> Result<Self, ()> {
		Self::new(from).ok_or(())
	}
}

impl From<Controller> for ffi::CController {
	fn from(from: Controller) -> Self {
		from.0
	}
}
