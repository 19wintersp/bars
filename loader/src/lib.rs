use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use bars_platform::api::{COMPATIBILITY, Exit, Init, InitContext, Version};
use bars_platform::{NAMESPACE, api_export_name};
use bars_update::{Binary, Package};

use anyhow::{Result, anyhow};
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

#[cfg(feature = "download")]
static BUNDLED_PACKAGE: Option<Package> = None;
#[cfg(not(feature = "download"))]
static BUNDLED_PACKAGE: Option<Package> = Some(Package::new(include_bytes!(
	env!("LOADER_BUNDLED_PACKAGE_PATH")
)));

static mut PLUGIN: Option<Plugin> = None;

#[unsafe(export_name = bars_euroscope::export_name!(init))]
unsafe extern "C" fn euroscope_init(pointer: *mut *mut c_void) {
	let _ = bars_tracing::init!();

	debug!("init");

	let (plugin_path, server_path) = match find_binaries(BUNDLED_PACKAGE.as_ref())
	{
		Ok(paths) => paths,
		Err(err) => {
			error!("failed to find binaries: {err}");

			#[cfg(not(feature = "download"))]
			return;

			#[cfg(feature = "download")]
			{
				debug!("downloading binaries");

				let res =
					download_binaries().and_then(|package| find_binaries(Some(&package)));
				match res {
					Ok(paths) => paths,
					Err(err) => {
						error!("failed to download/find binaries: {err}");
						return
					},
				}
			}
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
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
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
		debug!("calling plugin init");
		(plugin.init)(pointer, &raw const INIT_CONTEXT);
		debug!("storing library");
		PLUGIN = Some(plugin);
	}
}

#[unsafe(export_name = bars_euroscope::export_name!(exit))]
unsafe extern "C" fn euroscope_exit() {
	debug!("exit");

	unsafe {
		if let Some(plugin) = (&raw mut PLUGIN).as_mut().unwrap().take() {
			debug!("calling plugin exit");
			(plugin.exit)();
			debug!("unloading library");
			if let Err(err) = plugin.library.close() {
				error!("failed to close library: {err}");
			}
		}
	}

	debug!("exited");
}

fn find_binaries(package: Option<&Package>) -> Result<(PathBuf, PathBuf)> {
	let dir = bars_platform::dir::binaries()
		.ok_or(anyhow!("failed to identify binaries directory"))?;
	std::fs::create_dir_all(&dir)?;

	if let Some(package) = package {
		package.extract(&dir)?;
	}

	let find = |binary: &Binary| -> Result<PathBuf> {
		binary
			.find(&dir)?
			.into_iter()
			.max()
			.ok_or_else(|| anyhow!("missing file"))
			.inspect(|path| debug!("found {path:?}"))
	};

	Ok((find(&Binary::PLUGIN)?, find(&Binary::SERVER)?))
}

#[cfg(feature = "download")]
fn download_binaries() -> Result<Package> {
	use windows::Win32::UI::WindowsAndMessaging::{
		IDOK, MB_ICONINFORMATION, MB_OKCANCEL, MB_TASKMODAL, MessageBoxA,
	};
	use windows::core::s;

	if unsafe {
		MessageBoxA(
			None,
			s!(
				"The plugin appears to have been loaded for the first time on this \
				system. It will now attempt to complete the installation process by \
				downloading the necessary files from the server. If this fails, please \
				consult the logs."
			),
			s!("BARS loader"),
			MB_OKCANCEL | MB_ICONINFORMATION | MB_TASKMODAL,
		)
	} != IDOK
	{
		anyhow::bail!("user cancelled download");
	}

	Package::download_blocking()
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
