mod context;
mod plugin;
mod screen;

use std::ffi::{CStr, c_void};

use bars_config::Color;
use bars_euroscope::{ExportContext, Plugin, PluginOptions};
use bars_platform::api::InitContext;

const PLUGIN_OPTIONS: PluginOptions<'_> = PluginOptions {
	name: c"BARS",
	version: const {
		let s = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
		unsafe { CStr::from_bytes_with_nul_unchecked(s) }
	},
	author: c"Patrick Winters",
	copyright: c"GNU GPLv3",
};

const THEME_COLOR: Color = Color {
	r: 0x10,
	g: 0xb9,
	b: 0x81,
	a: 0xff,
};

fn init(ctx: &mut ExportContext) {
	let _ = bars_tracing::init!();

	bars_graphics::startup();

	ctx.register_plugin(Plugin::new(plugin::Plugin::new(), PLUGIN_OPTIONS));
}

fn exit() {
	bars_graphics::shutdown();
}

bars_euroscope::export!(init, exit);

#[unsafe(export_name = bars_platform::api_export_name!(init))]
unsafe extern "C" fn api_init(plugin: *mut *mut c_void, _: *const InitContext) {
	unsafe {
		_bars_euroscope_init(plugin);
	}
}

#[unsafe(export_name = bars_platform::api_export_name!(exit))]
unsafe extern "C" fn api_exit() {
	unsafe {
		_bars_euroscope_exit();
	}
}

const _: bars_platform::api::Init = api_init;
const _: bars_platform::api::Exit = api_exit;
