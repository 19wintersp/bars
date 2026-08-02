macro_rules! mangled_name {
	($class:ident$(::$member:ident)?) => {
		include!(concat!(
			env!("OUT_DIR"),
			"/mangled-name/",
			stringify!($class),
			$(
				"_",
				stringify!($member),
			)?
		))
	};
}

macro_rules! extern_virtual {
	(
		unsafe fn $class:ident::$member:ident $params:tt $( -> $return:ty )?
			$body:block
	) => {
		#[allow(non_snake_case)]
		#[unsafe(export_name = mangled_name!($class::$member))]
		unsafe extern "thiscall" fn $member $params $( -> $return )? $body
	};
}

macro_rules! uninit {
	() => {{
		#[allow(invalid_value, unused_unsafe)]
		unsafe { std::mem::MaybeUninit::uninit().assume_init() }
	}};
}

mod controller;
mod flight_plan;
mod plugin;
mod position;
mod radar_screen;
mod radar_target;
mod settings;

pub use controller::Controller;
pub use flight_plan::FlightPlan;
pub use plugin::*;
pub use position::Position;
pub use radar_screen::*;
pub use radar_target::RadarTarget;
pub use settings::*;

mod ffi {
	#[allow(
		dead_code,
		non_camel_case_types,
		non_snake_case,
		non_upper_case_globals,
		unsafe_op_in_unsafe_fn
	)]
	mod private {
		include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
	}

	macro_rules! const_assert_eq {
		( $lhs:expr, $rhs:expr ) => {
			[()][($lhs != $rhs) as usize];
		};
	}

	const _: () = {
		use std::any::Any;
		use std::mem::{align_of, size_of};

		const_assert_eq!(size_of::<PtrDyn>(), size_of::<*mut dyn Any>());
		const_assert_eq!(align_of::<PtrDyn>(), align_of::<*mut dyn Any>());
	};

	pub use private::root::EuroScopePlugIn::*;
	pub use private::root::safe::*;
	pub use private::root::{HDC, POINT, Plugin, PtrDyn, RECT, RadarScreen};
}

pub use ffi::{HDC as Hdc, POINT as Point, RECT as Area};

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum PopupListElementCheckbox {
	Unchecked = ffi::POPUP_ELEMENT_UNCHECKED,
	Checked = ffi::POPUP_ELEMENT_CHECKED,
	#[default]
	NoCheckbox = ffi::POPUP_ELEMENT_NO_CHECKBOX,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum ConnectionType {
	None = ffi::CONNECTION_TYPE_NO,
	Direct = ffi::CONNECTION_TYPE_DIRECT,
	Playback = ffi::CONNECTION_TYPE_PLAYBACK,
	Proxy = ffi::CONNECTION_TYPE_VIA_PROXY,
	SimulatorClient = ffi::CONNECTION_TYPE_SIMULATOR_CLIENT,
	SimulatorServer = ffi::CONNECTION_TYPE_SIMULATOR_SERVER,
	Sweatbox = ffi::CONNECTION_TYPE_SWEATBOX,
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum TagColor {
	#[default]
	Default = ffi::TAG_COLOR_DEFAULT,
	NonConcerned = ffi::TAG_COLOR_NON_CONCERNED,
	Notified = ffi::TAG_COLOR_NOTIFIED,
	Assumed = ffi::TAG_COLOR_ASSUMED,
	TransferToMeInitiated = ffi::TAG_COLOR_TRANSFER_TO_ME_INITIATED,
	Redundant = ffi::TAG_COLOR_REDUNDANT,
	Information = ffi::TAG_COLOR_INFORMATION,
	OngoingRequestFromMe = ffi::TAG_COLOR_ONGOING_REQUEST_FROM_ME,
	OngoingRequestToMe = ffi::TAG_COLOR_ONGOING_REQUEST_TO_ME,
	OngoingRequestAccepted = ffi::TAG_COLOR_ONGOING_REQUEST_ACCEPTED,
	OngoingRequestRefused = ffi::TAG_COLOR_ONGOING_REQUEST_REFUSED,
	Emergency = ffi::TAG_COLOR_EMERGENCY,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum RefreshPhase {
	Backdrop = ffi::REFRESH_PHASE_BACK_BITMAP,
	BeforeTags = ffi::REFRESH_PHASE_BEFORE_TAGS,
	AfterTags = ffi::REFRESH_PHASE_AFTER_TAGS,
	AfterLists = ffi::REFRESH_PHASE_AFTER_LISTS,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum MouseButton {
	Left = ffi::BUTTON_LEFT,
	Middle = ffi::BUTTON_MIDDLE,
	Right = ffi::BUTTON_RIGHT,
}

#[macro_export]
macro_rules! export_name {
	(init) => { "?EuroScopePlugInInit@@YAXPAPAVCPlugIn@EuroScopePlugIn@@@Z" };
	(exit) => { "?EuroScopePlugInExit@@YAXXZ" };
}

#[macro_export]
macro_rules! export {
	( $init:path $(, $exit:path)? $(,)? ) => {
		static mut _BARS_EUROSCOPE: ::std::option::Option<
			$crate::ExportContext,
		> = ::std::option::Option::None;

		#[unsafe(export_name = $crate::export_name!(init))]
		unsafe extern "C" fn _bars_euroscope_init(
			pointer: *mut *mut ::std::ffi::c_void,
		) {
			unsafe {
				_BARS_EUROSCOPE = Some(ExportContext::new(pointer.cast()));
				$init(_BARS_EUROSCOPE.as_mut().unwrap());
			}
		}

		#[unsafe(export_name = $crate::export_name!(exit))]
		unsafe extern "C" fn _bars_euroscope_exit() {
			unsafe { _BARS_EUROSCOPE.take() };
			$( $exit(); )?
		}
	};
}

pub struct ExportContext {
	plugin: Option<Plugin>,
	pointer: *mut *mut Plugin,
}

impl ExportContext {
	#[doc(hidden)]
	pub fn new(pointer: *mut *mut Plugin) -> Self {
		Self {
			plugin: None,
			pointer,
		}
	}

	pub fn register_plugin(&mut self, plugin: Plugin) {
		self.plugin = Some(plugin);
		unsafe { self.pointer.write(self.plugin.as_mut().unwrap()) };
	}
}
