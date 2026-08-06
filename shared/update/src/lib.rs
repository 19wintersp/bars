use std::borrow::Cow;
use std::fs::File;
use std::io::{ErrorKind, Result as IoResult};
use std::path::{Path, PathBuf};

use lzma_rust2::XzReader;
use tar::Archive;
use tracing::debug;

pub struct Binary {
	prefix: &'static str,
	suffix: &'static str,
}

impl Binary {
	pub const PLUGIN: Self = Self {
		prefix: "bars-plugin-core-",
		suffix: ".dll",
	};
	pub const SERVER: Self = Self {
		prefix: "bars-plugin-server-",
		suffix: ".exe",
	};

	pub fn find(&self, dir: impl AsRef<Path>) -> IoResult<Vec<PathBuf>> {
		std::fs::read_dir(dir)?
			.filter_map(|res| {
				res
					.map(|entry| {
						Some(entry)
							.filter(|entry| {
								entry.file_type().is_ok_and(|ty| ty.is_file())
									&& entry.file_name().to_str().is_some_and(|name| {
										name.starts_with(self.prefix) && name.ends_with(self.suffix)
									})
							})
							.map(|entry| entry.path())
					})
					.transpose()
			})
			.collect()
	}
}

pub struct Package {
	data: Cow<'static, [u8]>,
}

impl Package {
	pub const fn new(bytes: &'static [u8]) -> Self {
		Self {
			data: Cow::Borrowed(bytes),
		}
	}

	pub fn extract(&self, dest: impl AsRef<Path>) -> IoResult<()> {
		std::fs::create_dir_all(&dest)?;

		let decompressor = XzReader::<&[u8]>::new(&self.data, true);
		let mut archive = Archive::new(decompressor);

		let mut buf = dest.as_ref().to_path_buf();

		for entry in archive.entries()? {
			let mut entry = entry?;

			let path = entry.path()?;
			let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
				continue
			};

			let dest_name = name.trim_end_matches(".xz");
			buf.push(dest_name);

			let mut file = match File::create_new(&buf) {
				Ok(file) => file,
				Err(err) if err.kind() == ErrorKind::AlreadyExists => {
					debug!("skipping {buf:?}, exists");
					buf.pop();
					continue
				},
				Err(err) => Err(err)?,
			};

			if name.ends_with(".xz") {
				let mut reader = XzReader::new(entry, true);
				std::io::copy(&mut reader, &mut file)?;
			} else {
				std::io::copy(&mut entry, &mut file)?;
			}

			buf.pop();
		}

		Ok(())
	}
}

#[cfg(feature = "download")]
mod download {
	use anyhow::Result;
	use serde::Deserialize;
	use tracing::info;

	static RELEASE_URL: &str =
		"https://v2.stopbars.com/releases/latest?product=EuroScope-Plugin";

	impl crate::Package {
		pub async fn download() -> Result<Self> {
			let client = reqwest::Client::new();

			let release = client
				.get(RELEASE_URL)
				.send()
				.await?
				.error_for_status()?
				.json::<Release>()
				.await?;

			info!("downloaded release {} {}", release.product, release.version);

			Ok(Self {
				data: client
					.get(release.url)
					.send()
					.await?
					.bytes()
					.await?
					.to_vec()
					.into(),
			})
		}

		#[cfg(feature = "blocking")]
		pub fn download_blocking() -> Result<Self> {
			let client = reqwest::blocking::Client::new();

			let release = client
				.get(RELEASE_URL)
				.send()?
				.error_for_status()?
				.json::<Release>()?;

			info!("downloaded release {} {}", release.product, release.version);

			Ok(Self {
				data: client.get(release.url).send()?.bytes()?.to_vec().into(),
			})
		}
	}

	#[derive(Deserialize)]
	struct Release {
		product: String,
		version: String,
		#[serde(rename = "downloadUrl")]
		url: String,
	}
}
