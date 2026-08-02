use std::fs::File;
use std::io::IsTerminal;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow};
use chrono::Utc;
use tracing::{debug, error, info, instrument, warn};
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::ChronoUtc;
use tracing_subscriber::fmt::writer::Tee;

static FILE_SUFFIX: &str = ".log";

#[instrument(level = "debug")]
pub fn init(prefix: &str) -> Result<()> {
	let dir = bars_platform::dir::logs().ok_or(anyhow!("missing logs dir"))?;

	std::fs::create_dir_all(&dir)?;

	let date = Utc::now().format("%FT%H%M%S%.3fZ");
	let file_name = format!("{prefix}-{date}{FILE_SUFFIX}");
	let file = File::create(dir.join(file_name))?;

	#[cfg(debug_assertions)]
	let max_level = LevelFilter::TRACE;
	#[cfg(not(debug_assertions))]
	let max_level = LevelFilter::INFO;

	let subscriber = FmtSubscriber::builder()
		.with_ansi(false)
		.with_level(true)
		.with_max_level(max_level)
		.with_target(true)
		.with_thread_names(true)
		.with_timer(ChronoUtc::new("%T%.3fZ".into()));

	if std::io::stderr().is_terminal() {
		tracing::subscriber::set_global_default(
			subscriber.with_writer(Tee::new(file, std::io::stderr)).finish(),
		)?;
	} else {
		tracing::subscriber::set_global_default(
			subscriber.with_writer(file).finish(),
		)?;
	}

	info!("logging initialised");

	set_panic_hook();

	if let Err(err) = prune_log_files(&dir) {
		error!("prune logs: {err}");
	}

	Ok(())
}

#[macro_export]
macro_rules! init {
	() => {
		$crate::init(env!("CARGO_PKG_NAME"))
	};
}

fn prune_log_files(dir: &Path) -> Result<()> {
	const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

	for file in std::fs::read_dir(dir)? {
		let file = file?;

		let name = file.file_name();
		let Some(name) = name.to_str() else {
			warn!("skipped bad filename in logs dir");
			continue
		};
		if !name.ends_with(FILE_SUFFIX) {
			warn!("skipped non-log file in logs dir");
			continue
		}

		let path = file.path();
		if std::fs::metadata(&path)?.modified()?.elapsed()? > MAX_AGE {
			std::fs::remove_file(&path)?;
		}
	}

	Ok(())
}

fn set_panic_hook() {
	debug!("setting panic hook");

	let default_hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		let location = info.location().unwrap();
		let location = format!("{}:{}", location.file(), location.line());

		let err = Box::new(info.payload());

		if let Some(err) = err.downcast_ref::<&str>() {
			tracing::error!("panic at {location}: {err}");
		} else if let Some(err) = err.downcast_ref::<String>() {
			tracing::error!("panic at {location}: {err}");
		} else {
			tracing::error!("panic at {location}");
		}

		default_hook(&info);
	}));
}
