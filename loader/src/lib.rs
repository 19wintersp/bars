use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use bars_platform::NAMESPACE;
use bars_platform::api::{COMPATIBILITY, Exit, Init, InitContext, Version};

use anyhow::{Result, anyhow};
use bars_platform::api_export_name;
use libloading::Library;
use tracing::{debug, error, warn};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, CreateEventA};
use windows::core::PCSTR;

static INIT_CONTEXT: InitContext = InitContext {
	version: Version {
		version: env!("CARGO_PKG_VERSION"),
		compatibility: COMPATIBILITY,
	},
};

static FALLBACK_PLUGIN: &[u8] =
	include_bytes!(env!("PLUGIN_LOADER_FALLBACK_PLUGIN_PATH"));
static FALLBACK_SERVER: &[u8] =
	include_bytes!(env!("PLUGIN_LOADER_FALLBACK_SERVER_PATH"));

static PLUGIN_PREFIX: &str = "bars-plugin-core-";
static PLUGIN_SUFFIX: &str = ".dll";
static SERVER_PREFIX: &str = "bars-plugin-server-";
static SERVER_SUFFIX: &str = ".exe";
static FALLBACK_SERIAL: &str = "00000000";

static mut PLUGIN: Option<Plugin> = None;

#[unsafe(export_name = bars_euroscope::export_name!(init))]
unsafe extern "C" fn euroscope_init(pointer: *mut *mut c_void) {
	let _ = bars_tracing::init!();

	let (plugin_path, server_path) = match find_binaries() {
		Ok(paths) => paths,
		Err(err) => {
			error!("failed to find binaries: {err}");
			return
		},
	};

	debug!("creating lock handle");
	let lock_name = format!("Global\\{NAMESPACE}.loader.lock\0");
	unsafe {
		if CreateEventA(None, false, false, PCSTR(lock_name.as_ptr())).is_err() {
			warn!("failed to create lock");
		} else if GetLastError() == ERROR_ALREADY_EXISTS {
			debug!("lock exists already");
		}
	}

	debug!("starting server");
	if let Err(err) = Command::new(server_path)
		.creation_flags(CREATE_NO_WINDOW.0)
		.spawn()
	{
		error!("failed to spawn server: {err}");
		return
	}

	debug!("loading plugin");
	let plugin = match Plugin::load(&plugin_path) {
		Ok(plugin) => plugin,
		Err(err) => {
			error!("failed to load plugin: {err}");
			return
		},
	};

	unsafe {
		(plugin.init)(pointer, &raw const INIT_CONTEXT);
		PLUGIN = Some(plugin);
	}
}

#[unsafe(export_name = bars_euroscope::export_name!(exit))]
unsafe extern "C" fn euroscope_exit() {
	unsafe {
		if let Some(plugin) = (&raw mut PLUGIN).as_mut().unwrap().take() {
			(plugin.exit)();
			drop(plugin.library);
		}
	}
}

fn find_binaries() -> Result<(PathBuf, PathBuf)> {
	let dir = bars_platform::dir::binaries()
		.ok_or(anyhow!("failed to identify binaries directory"))?;
	std::fs::create_dir_all(&dir)?;

	let mut files = std::fs::read_dir(&dir)?
		.filter_map(|entry| entry.ok())
		.filter_map(|entry| entry.file_name().to_str().map(|s| s.to_string()))
		.collect::<Vec<_>>();
	files.sort();

	let find = |prefix: &str, suffix: &str, fallback: &[u8]| -> Result<PathBuf> {
		let file = files
			.iter()
			.rfind(|name| name.starts_with(prefix) && name.ends_with(suffix));
		let path = dir.join(
			file
				.cloned()
				.unwrap_or_else(|| format!("{prefix}{FALLBACK_SERIAL}{suffix}")),
		);

		if file.is_some() {
			debug!("found {path:?}");
		} else {
			debug!("creating fallback at {path:?}");
			std::fs::write(&path, fallback)?;
		}

		Ok(path)
	};

	Ok((
		find(PLUGIN_PREFIX, PLUGIN_SUFFIX, FALLBACK_PLUGIN)?,
		find(SERVER_PREFIX, SERVER_SUFFIX, FALLBACK_SERVER)?,
	))
}

struct Plugin {
	library: Library,
	init: Init,
	exit: Exit,
}

impl Plugin {
	fn load(path: &PathBuf) -> Result<Self> {
		unsafe {
			let library = Library::new(path)?;
			Ok(Self {
				init: *library.get::<Init>(api_export_name!(init))?,
				exit: *library.get::<Exit>(api_export_name!(exit))?,
				library,
			})
		}
	}
}
