use crate::{
	Area, ConnectionType, FlightPlan, Point, PopupListElementCheckbox,
	RadarScreen, RadarTarget, SettingStore, TagColor, ffi,
};

use std::ffi::{CStr, c_char};

static EMPTY_STRING: &'static CStr = c"";

#[derive(Debug)]
#[repr(transparent)]
pub struct Plugin(ffi::Plugin);

impl Plugin {
	pub fn new(handler: impl PluginHandler, options: PluginOptions) -> Self {
		let handler: Box<dyn PluginHandler> = Box::new(handler);

		let mut ctx = Self(unsafe {
			ffi::Plugin::new(
				options.name.as_ptr(),
				options.version.as_ptr(),
				options.author.as_ptr(),
				options.copyright.as_ptr(),
				std::mem::transmute(Box::into_raw(handler)),
			)
		});

		unsafe { ctx.handler() }.init(&mut ctx);

		ctx
	}

	pub fn register_display_type(
		&mut self,
		name: &CStr,
		options: DisplayTypeOptions,
	) {
		unsafe {
			self.0._base.RegisterDisplayType(
				name.as_ptr(),
				options.radar_content,
				options.geographic,
				options.serializable,
				options.creatable,
			);
		}
	}

	pub fn register_tag_item(&mut self, name: &CStr, code: i32) {
		unsafe {
			self.0._base.RegisterTagItemType(name.as_ptr(), code);
		}
	}

	pub fn register_tag_function(&mut self, name: &CStr, code: i32) {
		unsafe {
			self.0._base.RegisterTagItemFunction(name.as_ptr(), code);
		}
	}

	pub fn open_popup_input(&mut self, value: &CStr, function: i32, area: Area) {
		unsafe {
			self.0._base.OpenPopupEdit(area, function, value.as_ptr());
		}
	}

	pub fn open_popup_list(&mut self, title: &CStr, wide: bool, area: Area) {
		unsafe {
			self
				.0
				._base
				.OpenPopupList(area, title.as_ptr(), wide as i32 + 1);
		}
	}

	pub fn add_popup_list_item(
		&mut self,
		title: &CStr,
		value: Option<&CStr>,
		function: i32,
		options: PopupListItemOptions,
	) {
		unsafe {
			self.0._base.AddPopupListElement(
				title.as_ptr(),
				value
					.map(|value| value.as_ptr())
					.unwrap_or(EMPTY_STRING.as_ptr()),
				function,
				options.selected,
				options.checkbox as i32,
				options.disabled,
				options.sticky,
			);
		}
	}

	pub fn connection_type(&self) -> ConnectionType {
		unsafe { std::mem::transmute(self.0._base.GetConnectionType()) }
	}

	pub fn selected_flight_plan(&self) -> Option<FlightPlan> {
		unsafe {
			let mut out = uninit!();
			self.0.SafeFlightPlanSelectASEL(&raw mut out);
			out.into()
		}
	}

	pub fn get_flight_plan(&self, callsign: &CStr) -> Option<FlightPlan> {
		unsafe {
			let mut out = uninit!();
			self.0.SafeFlightPlanSelect(callsign.as_ptr(), &raw mut out);
			out.into()
		}
	}

	pub fn flight_plans(&self) -> impl Iterator<Item = FlightPlan> + '_ {
		struct Iter<'a> {
			plugin: &'a ffi::Plugin,
			current: Option<FlightPlan>,
		}

		impl Iterator for Iter<'_> {
			type Item = FlightPlan;

			fn next(&mut self) -> Option<FlightPlan> {
				let mut next = uninit!();

				if let Some(fp) = self.current {
					unsafe {
						self
							.plugin
							.SafeFlightPlanSelectNext(fp.into(), &raw mut next);
					}
				} else {
					unsafe {
						self.plugin.SafeFlightPlanSelectFirst(&raw mut next);
					}
				}

				self.current = next.into();
				self.current
			}
		}

		Iter {
			plugin: &self.0,
			current: None,
		}
		.fuse()
	}

	pub fn selected_radar_target(&self) -> Option<RadarTarget> {
		unsafe {
			let mut out = uninit!();
			self.0.SafeRadarTargetSelectASEL(&raw mut out);
			out.into()
		}
	}

	pub fn get_radar_target(&self, callsign: &CStr) -> Option<RadarTarget> {
		unsafe {
			let mut out = uninit!();
			self
				.0
				.SafeRadarTargetSelect(callsign.as_ptr(), &raw mut out);
			out.into()
		}
	}

	pub fn radar_targets(&self) -> impl Iterator<Item = RadarTarget> + '_ {
		struct Iter<'a> {
			plugin: &'a ffi::Plugin,
			current: Option<RadarTarget>,
		}

		impl Iterator for Iter<'_> {
			type Item = RadarTarget;

			fn next(&mut self) -> Option<RadarTarget> {
				let mut next = uninit!();

				if let Some(rt) = self.current {
					unsafe {
						self
							.plugin
							.SafeRadarTargetSelectNext(rt.into(), &raw mut next);
					}
				} else {
					unsafe {
						self.plugin.SafeRadarTargetSelectFirst(&raw mut next);
					}
				}

				self.current = next.into();
				self.current
			}
		}

		Iter {
			plugin: &self.0,
			current: None,
		}
		.fuse()
	}

	pub fn display_message(
		&mut self,
		sender: &CStr,
		message: &CStr,
		options: MessageOptions,
	) {
		unsafe {
			let name = self.0._base.GetPlugInName();
			self.0._base.DisplayUserMessage(
				name,
				sender.as_ptr(),
				message.as_ptr(),
				options.handler != MessageHandler::None,
				options.status != MessageStatus::Read,
				options.status == MessageStatus::ForceUnread,
				options.handler == MessageHandler::Flashing,
				options.confirmable,
			);
		}
	}

	unsafe fn handler(&self) -> &'static mut dyn PluginHandler {
		unsafe {
			std::mem::transmute::<_, *mut dyn PluginHandler>(self.0.handler)
				.as_mut()
				.unwrap()
		}
	}

	unsafe fn from_ffi_mut<'a>(this: *mut ffi::Plugin) -> &'a mut Self {
		unsafe { this.cast::<Self>().as_mut_unchecked() }
	}
}

impl Drop for Plugin {
	fn drop(&mut self) {
		let _ = unsafe { Box::from_raw(self.handler()) };
	}
}

impl SettingStore for Plugin {
	fn save_raw_setting(&mut self, key: &CStr, value: &CStr, description: &CStr) {
		unsafe {
			self.0._base.SaveDataToSettings(
				key.as_ptr(),
				description.as_ptr(),
				value.as_ptr(),
			);
		}
	}

	fn load_raw_setting(&mut self, key: &CStr) -> Option<&CStr> {
		let value = unsafe { self.0._base.GetDataFromSettings(key.as_ptr()) };
		(!value.is_null()).then(|| unsafe { CStr::from_ptr(value) })
	}
}

pub trait PluginHandler {
	fn init(&mut self, ctx: &mut Plugin) {
		let _ = ctx;
	}

	fn compile_command(&mut self, ctx: &mut Plugin, command: &CStr) -> bool {
		let _ = (ctx, command);
		false
	}

	fn create_radar_screen(
		&mut self,
		ctx: &mut Plugin,
		name: &CStr,
		options: DisplayTypeOptions,
	) -> Option<RadarScreen> {
		let _ = (ctx, name, options);
		None
	}

	fn get_tag_item(
		&mut self,
		ctx: &mut Plugin,
		code: i32,
		flight_plan: Option<FlightPlan>,
		radar_target: Option<RadarTarget>,
		tag_content: &mut TagContent,
	) {
		let _ = (ctx, code, flight_plan, radar_target, tag_content);
	}

	fn call_tag_function(
		&mut self,
		ctx: &mut Plugin,
		code: i32,
		string: Option<&CStr>,
		point: Point,
		area: Area,
	) {
		let _ = (ctx, code, string, point, area);
	}

	fn tick(&mut self, ctx: &mut Plugin, time: i32) {
		let _ = (ctx, time);
	}
}

extern_virtual!(
	unsafe fn Plugin::OnCompileCommand(
		this: *mut ffi::Plugin,
		command: *const c_char,
	) -> bool {
		unsafe {
			let command = CStr::from_ptr(command);

			let ctx = Plugin::from_ffi_mut(this);
			ctx.handler().compile_command(ctx, command)
		}
	}
);

extern_virtual!(
	unsafe fn Plugin::OnRadarScreenCreated(
		this: *mut ffi::Plugin,
		name: *const c_char,
		radar_content: bool,
		geographic: bool,
		serializable: bool,
		creatable: bool,
	) -> *mut ffi::RadarScreen {
		unsafe {
			let name = CStr::from_ptr(name);
			let options = DisplayTypeOptions {
				radar_content,
				geographic,
				serializable,
				creatable,
			};

			let ctx = Plugin::from_ffi_mut(this);
			let screen = ctx.handler().create_radar_screen(ctx, name, options);

			if let Some(screen) = screen {
				screen.into_ffi_mut()
			} else {
				std::ptr::null_mut()
			}
		}
	}
);

extern_virtual!(
	unsafe fn Plugin::OnGetTagItem(
		this: *mut ffi::Plugin,
		flight_plan: ffi::CFlightPlan,
		radar_target: ffi::CRadarTarget,
		code: i32,
		_tag_data: i32,
		string: *mut [c_char; 16],
		color: *mut i32,
		rgb: *mut i32,
		font_size: *mut f64,
	) {
		let mut content = TagContent { string, color, rgb, font_size };

		unsafe {
			let ctx = Plugin::from_ffi_mut(this);
			ctx
				.handler()
				.get_tag_item(
					ctx, code, flight_plan.into(), radar_target.into(), &mut content,
				);
		}
	}
);

extern_virtual!(
	unsafe fn Plugin::OnFunctionCall(
		this: *mut ffi::Plugin,
		code: i32,
		string: *const c_char,
		point: Point,
		area: Area,
	) {
		unsafe {
			let string = (!string.is_null()).then(|| CStr::from_ptr(string));

			let ctx = Plugin::from_ffi_mut(this);
			ctx.handler().call_tag_function(ctx, code, string, point, area);
		}
	}
);

extern_virtual!(
	unsafe fn Plugin::OnTimer(this: *mut ffi::Plugin, time: i32) {
		unsafe {
			let ctx = Plugin::from_ffi_mut(this);
			ctx.handler().tick(ctx, time);
		}
	}
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PluginOptions<'a> {
	pub name: &'static CStr,
	pub version: &'a CStr,
	pub author: &'a CStr,
	pub copyright: &'a CStr,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DisplayTypeOptions {
	pub radar_content: bool,
	pub geographic: bool,
	pub serializable: bool,
	pub creatable: bool,
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MessageOptions {
	pub handler: MessageHandler,
	pub status: MessageStatus,
	pub confirmable: bool,
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessageHandler {
	None,
	#[default]
	Normal,
	Flashing,
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessageStatus {
	Read,
	#[default]
	Unread,
	ForceUnread,
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PopupListItemOptions {
	pub selected: bool,
	pub disabled: bool,
	pub sticky: bool,
	pub checkbox: PopupListElementCheckbox,
}

#[derive(Debug)]
pub struct TagContent {
	string: *mut [c_char; 16],
	color: *mut i32,
	rgb: *mut i32,
	font_size: *mut f64,
}

impl TagContent {
	pub fn set_string(&mut self, string: &CStr) {
		let dst = self.string as *mut c_char;
		let len = string.count_bytes().min(15);

		unsafe {
			std::ptr::copy(string.as_ptr(), dst, len);
			dst.add(len).write(0);
		}
	}

	pub fn set_color(&mut self, color: TagColor) {
		unsafe {
			self.color.write(color as i32);
		}
	}

	pub fn set_rgb(&mut self, rgb: u32) {
		let [_, r, g, b] = rgb.to_be_bytes();
		let colorref = ((b as i32) << 16) | ((g as i32) << 8) | r as i32;

		unsafe {
			self.color.write(ffi::TAG_COLOR_RGB_DEFINED);
			self.rgb.write(colorref);
		}
	}

	pub fn set_font_size(&mut self, font_size: f64) {
		unsafe {
			self.font_size.write(font_size);
		}
	}
}
