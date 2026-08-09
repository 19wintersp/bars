use std::path::PathBuf;

pub fn base() -> Option<PathBuf> {
	#[cfg(windows)]
	let mut dir = PathBuf::from(std::env::var_os("APPDATA")?);

	#[cfg(all(not(windows), debug_assertions))]
	let mut dir = PathBuf::from("/tmp");

	dir.push(crate::NAMESPACE);
	Some(dir)
}

macro_rules! dir {
	($ident:ident, $dir:literal) => {
		pub fn $ident() -> Option<PathBuf> {
			let mut dir = base()?;
			dir.push($dir);
			Some(dir)
		}
	};
}

dir!(binaries, "bin");
dir!(logs, "log");
