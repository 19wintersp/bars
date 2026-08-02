use std::path::PathBuf;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
	pub api: ApiSettings,
}

impl Settings {
	pub fn load() -> Result<Self> {
		Ok(toml::from_str(&std::fs::read_to_string(
			&Self::file_path()?,
		)?)?)
	}

	pub fn save(&self) -> Result<()> {
		std::fs::write(&Self::file_path()?, toml::to_string(self)?)?;
		Ok(())
	}

	fn file_path() -> Result<PathBuf> {
		bars_platform::dir::base()
			.ok_or(anyhow!("unable to identify user directory"))
			.map(|mut path| {
				path.push("config.toml");
				path
			})
	}
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiSettings {
	pub token: Option<String>,
	pub api: Option<String>,
	pub cdn: Option<String>,
}
