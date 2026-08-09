use std::ffi::c_void;

pub const COMPATIBILITY: usize = 1;

pub type Init = unsafe extern "C" fn(*mut *mut c_void, *const InitContext);
pub type Exit = unsafe extern "C" fn();

#[macro_export]
macro_rules! api_export_name {
	(init) => { "bars_plugin_api_init" };
	(exit) => { "bars_plugin_api_exit" };
}

#[repr(C)]
pub struct InitContext {
	pub version: Version,
}

pub struct Version {
	pub version: &'static str,
	pub compatibility: usize,
}
