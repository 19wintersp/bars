use crate::THEME_COLOR;
use crate::context::{ClientState, Context};
use crate::screen::Screen;

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::rc::Rc;

use bars_euroscope::{
	ConnectionType, DisplayTypeOptions, FlightPlan, Plugin as PluginContext,
	PluginHandler, RadarScreen, RadarTarget, TagContent,
};
use bars_ipc::ConnectionTarget as NetworkConnection;

const DISPLAY_TYPE_NAME: &CStr = c"Lighting Control Panel";

const TAG_ITEM_NAME: &CStr = c"Pilot connectivity";
const TAG_ITEM_CODE: i32 = 101;

trait PluginWrapper {
	fn ctx(&mut self) -> &mut PluginContext;

	fn display_basic_message(&mut self, message: &str) {
		let message = CString::new(message.replace('\0', "")).unwrap();
		self
			.ctx()
			.display_message(c"", &message, Default::default());
	}
}

impl PluginWrapper for PluginContext {
	fn ctx(&mut self) -> &mut PluginContext {
		self
	}
}

pub struct Plugin {
	context: Rc<RefCell<Context>>,
	connection: ConnectionType,
}

impl Plugin {
	pub fn new() -> Self {
		Self {
			context: Rc::new(RefCell::new(Context::new())),
			connection: ConnectionType::None,
		}
	}
}

impl PluginHandler for Plugin {
	fn init(&mut self, ctx: &mut PluginContext) {
		ctx.register_display_type(
			DISPLAY_TYPE_NAME,
			DisplayTypeOptions {
				creatable: true,
				radar_content: false,
				geographic: false,
				serializable: true,
			},
		);
		ctx.register_tag_item(TAG_ITEM_NAME, TAG_ITEM_CODE);
	}

	fn compile_command(
		&mut self,
		ctx: &mut PluginContext,
		command: &CStr,
	) -> bool {
		let command = command.to_string_lossy();
		let parts = command.trim().split_ascii_whitespace().collect::<Vec<_>>();

		if parts
			.get(0)
			.is_some_and(|s| s.eq_ignore_ascii_case(".bars"))
		{
			let part = parts.get(1).map(|part| part.to_ascii_lowercase());
			match part.as_ref().map(|part| part.as_str()) {
				None | Some("help") => ctx.display_basic_message(
					"Available commands: auth, connect, start, version",
				),
				Some("version") => ctx.display_basic_message(concat!(
					env!("CARGO_PKG_NAME"),
					" ",
					env!("CARGO_PKG_VERSION")
				)),
				Some("auth") => {
					self.context.borrow().authenticate(parts.get(2).map(|s| *s));
				},
				Some("connect") => {
					let context = self.context.borrow();

					let connection = match context.network_state() {
						NetworkConnection::None => NetworkConnection::Network,
						NetworkConnection::Network => NetworkConnection::None,
					};
					context.connect_network(connection);
				},
				Some("local") => {
					ctx.display_basic_message("Not yet implemented");
				},
				Some("start") => {
					self.context.borrow_mut().connect();
				},
				Some(other) => ctx.display_basic_message(&format!(
					"Error: unrecognised command: {other:?}"
				)),
			}

			true
		} else {
			false
		}
	}

	fn create_radar_screen(
		&mut self,
		_ctx: &mut PluginContext,
		name: &CStr,
		options: DisplayTypeOptions,
	) -> Option<RadarScreen> {
		if options.geographic {
			Some(RadarScreen::new(Screen::new(self.context.clone(), true)))
		} else if name == DISPLAY_TYPE_NAME {
			Some(RadarScreen::new(Screen::new(self.context.clone(), false)))
		} else {
			None
		}
	}

	fn get_tag_item(
		&mut self,
		_ctx: &mut PluginContext,
		code: i32,
		flight_plan: Option<FlightPlan>,
		radar_target: Option<RadarTarget>,
		tag_content: &mut TagContent,
	) {
		if code == TAG_ITEM_CODE {
			let Some(callsign) = flight_plan
				.map(|fp| fp.callsign().to_string_lossy().into_owned())
				.or(
					radar_target.map(|rt| rt.callsign().to_string_lossy().into_owned()),
				)
			else {
				return
			};

			if self.context.borrow().is_pilot_connected(&callsign) {
				tag_content.set_string(c"Y");
				tag_content.set_rgb(u32::from_be_bytes([
					0,
					THEME_COLOR.r,
					THEME_COLOR.g,
					THEME_COLOR.b,
				]));
			} else {
				tag_content.set_string(c"-");
				tag_content.set_color(Default::default());
			}
		}
	}

	fn tick(&mut self, ctx: &mut PluginContext, _time: i32) {
		let state = self.context.borrow_mut().tick();

		for message in state.user_messages {
			ctx.display_basic_message(&message);
		}

		let connection = ctx.connection_type();
		if std::mem::replace(&mut self.connection, connection)
			== ConnectionType::None
			&& connection == ConnectionType::Direct
		{
			self
				.context
				.borrow()
				.connect_network(NetworkConnection::Network);
		}

		if state.client_state == ClientState::Disconnected {
			ctx.display_basic_message(
				"Error: internal server disconnected; run '.bars start' to reconnect",
			);
		}
	}
}
