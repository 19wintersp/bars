use std::borrow::Cow;

use anyhow::Result;
use http::uri::{Authority, Scheme, Uri};

#[derive(Clone)]
pub struct Base {
	secure: bool,
	authority: Authority,
	path: Cow<'static, str>,
}

impl Base {
	pub const fn default_api() -> Self {
		Self {
			secure: true,
			authority: Authority::from_static("v2.stopbars.com"),
			path: Cow::Borrowed("/"),
		}
	}

	pub const fn default_cdn() -> Self {
		Self {
			secure: true,
			authority: Authority::from_static("cdn.stopbars.com"),
			path: Cow::Borrowed("/"),
		}
	}

	pub fn new(base: &str) -> Option<Self> {
		let has_scheme = base.contains("://");
		let has_path = base.ends_with("/");

		let uri: Uri = if has_scheme && has_path {
			base.parse().ok()?
		} else {
			format!(
				"{}{base}{}",
				if has_scheme { "" } else { "https://" },
				if has_path { "" } else { "/" }
			)
			.parse()
			.ok()?
		};

		let parts = uri.into_parts();

		Some(Self {
			secure: parts.scheme.is_some_and(|scheme| scheme == Scheme::HTTPS),
			authority: parts.authority?,
			path: Cow::Owned(
				parts
					.path_and_query
					.as_ref()
					.map(|pq| pq.path())
					.unwrap_or("")
					.into(),
			),
		})
	}

	pub fn create_http_uri(&self, path: &str) -> Result<Uri> {
		self.create_uri(
			if self.secure {
				Scheme::HTTPS
			} else {
				Scheme::HTTP
			},
			path,
		)
	}

	pub fn create_ws_uri(&self, path: &str) -> Result<Uri> {
		self.create_uri(
			if self.secure {
				"wss".parse().unwrap()
			} else {
				"ws".parse().unwrap()
			},
			path,
		)
	}

	fn create_uri(&self, scheme: Scheme, path: &str) -> Result<Uri> {
		Ok(
			Uri::builder()
				.scheme(scheme)
				.authority(self.authority.as_ref())
				.path_and_query(format!(
					"{}{}",
					self.path,
					path.trim_start_matches('/')
				))
				.build()?,
		)
	}
}
