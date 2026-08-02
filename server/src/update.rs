use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use reqwest::Client;
use serde::Deserialize;

static REPO: &str = "19wintersp/bars";

pub struct Update {
	dir: PathBuf,
	files: Vec<DownloadedFile>,
}

struct DownloadedFile {
	name: String,
	data: Vec<u8>,
}

impl Update {
	pub async fn begin() -> Result<Self> {
		let dir = bars_platform::dir::binaries()
			.ok_or(anyhow!("failed to identify update destination"))?;
		std::fs::create_dir_all(&dir)?;

		let client = Client::new();

		let mut releases = client
			.get(format!("https://api.github.com/repos/{REPO}/releases"))
			.send()
			.await?
			.error_for_status()?
			.json::<Vec<Release>>()
			.await?;

		if releases.is_empty() {
			bail!("no releases exist");
		}

		let mut this = Self {
			dir,
			files: Vec::new(),
		};

		for asset in releases.swap_remove(0).assets {
			this.dir.push(&asset.name);
			if !this.dir.try_exists()? {
				this.files.push(DownloadedFile {
					name: asset.name,
					data: client.get(asset.url).send().await?.bytes().await?.to_vec(),
				});
			}
			this.dir.pop();
		}

		Ok(this)
	}

	pub fn commit(mut self) -> Result<()> {
		for file in self.files {
			self.dir.push(&file.name);
			std::fs::write(&self.dir, file.data)?;
			self.dir.pop();
		}

		Ok(())
	}
}

#[derive(Deserialize)]
struct Release {
	assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
	name: String,
	#[serde(rename = "browser_download_url")]
	url: String,
}
