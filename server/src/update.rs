use std::path::PathBuf;

use bars_update::Package;

use anyhow::{Result, anyhow};

pub struct Update {
	dir: PathBuf,
	package: Package,
}

impl Update {
	pub async fn begin() -> Result<Self> {
		let dir = bars_platform::dir::binaries()
			.ok_or(anyhow!("failed to identify update destination"))?;
		std::fs::create_dir_all(&dir)?;

		Ok(Self {
			dir,
			package: Package::download().await?,
		})
	}

	pub fn commit(self) -> Result<()> {
		self.package.extract(self.dir)?;

		// remove binaries that are older than the newest binary with an mtime at
		// least one day ago

		Ok(())
	}
}
