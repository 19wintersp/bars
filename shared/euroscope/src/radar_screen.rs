use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::{CStr, c_char};

use crate::{
	Area, Hdc, MouseButton, Plugin, Point, Position, RefreshPhase, SettingStore,
	ffi,
};

thread_local! {
	static RADAR_SCREENS: RefCell<HashSet<*mut RadarScreen>> =
		RefCell::new(HashSet::new());
}

// EuroScope does not reliably destruct radar screens when closing the
// application entirely, so we must do so ourselves when shutting down.
pub(crate) fn exit() {
	for screen in RADAR_SCREENS.take() {
		unsafe {
			RadarScreen::drop_ffi_mut(screen.cast());
		}
	}
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RadarScreen(ffi::RadarScreen);

impl RadarScreen {
	pub fn new(handler: impl RadarScreenHandler) -> Self {
		let handler: Box<dyn RadarScreenHandler> = Box::new(handler);

		Self(unsafe {
			ffi::RadarScreen::new(std::mem::transmute(Box::into_raw(handler)))
		})
	}

	pub fn plugin(&mut self) -> &mut Plugin {
		unsafe { self.0._base.m_pPlugIn.cast::<Plugin>().as_mut().unwrap() }
	}

	pub fn toolbar_area(&mut self) -> Area {
		unsafe { self.0._base.GetToolbarArea() }
	}

	pub fn radar_area(&mut self) -> Area {
		unsafe { self.0._base.GetRadarArea() }
	}

	pub fn chat_area(&mut self) -> Area {
		unsafe { self.0._base.GetChatArea() }
	}

	pub fn unproject(&mut self, point: Point) -> Position {
		unsafe { self.0._base.ConvertCoordFromPixelToPosition(point).into() }
	}

	pub fn project(&mut self, position: Position) -> Point {
		unsafe {
			let mut out = uninit!();
			self
				.0
				.SafeConvertCoordFromPositionToPixel(position.into(), &raw mut out);
			out
		}
	}

	pub fn request_refresh(&mut self) {
		unsafe {
			self.0._base.RequestRefresh();
		}
	}

	pub fn request_backdrop_refresh(&mut self) {
		unsafe {
			self.0._base.RefreshMapContent();
		}
	}

	pub fn add_screen_object(
		&mut self,
		object: ScreenObject,
		draggable: bool,
		message: &CStr,
	) {
		unsafe {
			self.0._base.AddScreenObject(
				object.group,
				object.id.as_ptr(),
				object.area,
				draggable,
				message.as_ptr(),
			);
		}
	}

	pub fn bounds(&mut self) -> RadarBounds {
		unsafe {
			let mut bounds: RadarBounds = uninit!();

			self.0._base.GetDisplayArea(
				(&raw mut bounds.bottom_left) as *mut _,
				(&raw mut bounds.top_right) as *mut _,
			);

			bounds
		}
	}

	pub fn set_bounds(&mut self, bounds: RadarBounds) {
		unsafe {
			self
				.0
				._base
				.SetDisplayArea(bounds.bottom_left.into(), bounds.top_right.into());
		}
	}

	unsafe fn handler(&self) -> &'static mut dyn RadarScreenHandler {
		unsafe {
			std::mem::transmute::<_, *mut dyn RadarScreenHandler>(self.0.handler)
				.as_mut()
				.unwrap()
		}
	}

	unsafe fn from_ffi_mut<'a>(this: *mut ffi::RadarScreen) -> &'a mut Self {
		unsafe { this.cast::<Self>().as_mut_unchecked() }
	}

	pub(crate) fn into_ffi_mut(self) -> *mut ffi::RadarScreen {
		let ptr = Box::into_raw(Box::new(self));
		RADAR_SCREENS.with_borrow_mut(|ptrs| ptrs.insert(ptr));
		ptr.cast()
	}

	unsafe fn drop_ffi_mut(this: *mut ffi::RadarScreen) {
		unsafe {
			let _ = Box::<Self>::from_raw(this.cast());
		}
	}
}

impl Drop for RadarScreen {
	fn drop(&mut self) {
		let _ = unsafe { Box::from_raw(self.handler()) };
	}
}

impl SettingStore for RadarScreen {
	fn save_raw_setting(&mut self, key: &CStr, value: &CStr, description: &CStr) {
		unsafe {
			self.0._base.SaveDataToAsr(
				key.as_ptr(),
				description.as_ptr(),
				value.as_ptr(),
			);
		}
	}

	fn load_raw_setting(&mut self, key: &CStr) -> Option<&CStr> {
		let value = unsafe { self.0._base.GetDataFromAsr(key.as_ptr()) };
		(!value.is_null()).then(|| unsafe { CStr::from_ptr(value) })
	}
}

pub trait RadarScreenHandler {
	fn init(&mut self, ctx: &mut RadarScreen) {
		let _ = ctx;
	}

	fn refresh(&mut self, ctx: &mut RadarScreen, hdc: Hdc, phase: RefreshPhase) {
		let _ = (ctx, hdc, phase);
	}

	fn compile_command(&mut self, ctx: &mut RadarScreen, command: &CStr) -> bool {
		let _ = (ctx, command);
		false
	}

	fn mouse_event(
		&mut self,
		ctx: &mut RadarScreen,
		event: MouseEvent,
		position: Point,
		object: ScreenObject,
	) {
		let _ = (ctx, event, position, object);
	}

	fn call_tag_function(
		&mut self,
		ctx: &mut RadarScreen,
		code: i32,
		string: Option<&CStr>,
		point: Point,
		area: Area,
	) {
		let _ = (ctx, code, string, point, area);
	}
}

extern_virtual!(
	unsafe fn RadarScreen::OnAsrContentLoaded(
		this: *mut ffi::RadarScreen,
		_loaded: bool,
	) {
		unsafe {
			let ctx = RadarScreen::from_ffi_mut(this);
			ctx.handler().init(ctx);
		}
	}
);

extern_virtual!(
	unsafe fn RadarScreen::OnRefresh(
		this: *mut ffi::RadarScreen,
		hdc: Hdc,
		phase: RefreshPhase,
	) {
		unsafe {
			let ctx = RadarScreen::from_ffi_mut(this);
			ctx.handler().refresh(ctx, hdc, phase);
		}
	}
);

extern_virtual!(
	unsafe fn RadarScreen::OnAsrContentToBeClosed(this: *mut ffi::RadarScreen) {
		RADAR_SCREENS.with_borrow_mut(|ptrs| ptrs.remove(&this.cast()));

		unsafe {
			RadarScreen::drop_ffi_mut(this);
		}
	}
);

extern_virtual!(
	unsafe fn RadarScreen::OnCompileCommand(
		this: *mut ffi::RadarScreen,
		command: *const c_char,
	) -> bool {
		unsafe {
			let command = CStr::from_ptr(command);

			let ctx = RadarScreen::from_ffi_mut(this);
			ctx.handler().compile_command(ctx, command)
		}
	}
);

extern_virtual!(
	unsafe fn RadarScreen::OnFunctionCall(
		this: *mut ffi::RadarScreen,
		code: i32,
		string: *const c_char,
		point: Point,
		area: Area,
	) {
		unsafe {
			let string = (!string.is_null()).then(|| CStr::from_ptr(string));

			let ctx = RadarScreen::from_ffi_mut(this);
			ctx.handler().call_tag_function(ctx, code, string, point, area);
		}
	}
);

macro_rules! impl_mouse_events {
	( $( $ident:ident ( $( $params:tt )* ) => $event:expr ),+ $(,)? ) => {
		$(
			extern_virtual!(
				unsafe fn RadarScreen::$ident(
					this: *mut ffi::RadarScreen,
					object_type: i32,
					object_id: *const c_char,
					point: Point,
					area: Area,
					$( $params )*
				) {
					unsafe {
						let ctx = RadarScreen::from_ffi_mut(this);
						ctx.handler().mouse_event(
							ctx,
							$event,
							point,
							ScreenObject {
								group: object_type,
								id: CStr::from_ptr(object_id),
								area,
							},
						);
					}
				}
			);
		)+
	};
}

impl_mouse_events!(
	OnOverScreenObject() => MouseEvent::Over,
	OnButtonDownScreenObject(button: MouseButton) => MouseEvent::Down(button),
	OnButtonUpScreenObject(button: MouseButton) => MouseEvent::Up(button),
	OnClickScreenObject(button: MouseButton) => MouseEvent::Click(button),
	OnDoubleClickScreenObject(button: MouseButton) => {
		MouseEvent::DoubleClick(button)
	},
	OnMoveScreenObject(released: bool) => {
		if released {
			MouseEvent::DragEnd
		} else {
			MouseEvent::Drag
		}
	},
);

#[derive(Clone, Copy, Debug)]
pub struct RadarBounds {
	pub bottom_left: Position,
	pub top_right: Position,
}

#[derive(Clone, Copy, Debug)]
pub struct ScreenObject<'a> {
	/// Undocumented EuroScope behaviour: if this value is set to zero, the object
	/// will be transparent to right-click pan and re-centre operations, and will
	/// also not generate Up, Click, DoubleClick, or DragEnd events.
	pub group: i32,
	pub id: &'a CStr,
	pub area: Area,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum MouseEvent {
	Over,
	Down(MouseButton),
	Up(MouseButton),
	Click(MouseButton),
	DoubleClick(MouseButton),
	Drag,
	DragEnd,
}
