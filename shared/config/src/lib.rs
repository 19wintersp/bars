mod aerodrome;
mod map;

use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::io::{Error as IoError, Read, Write};
use std::marker::PhantomData;

use bincode::config::Configuration as BincodeConfig;
use bincode::error::{DecodeError, EncodeError};
use bincode::{Decode, Encode};

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;

pub use aerodrome::*;
pub use map::*;

static MAGIC: &[u8] = b"\xffBARS\x13eu";

const BINCODE_CONFIG: BincodeConfig = bincode::config::standard();

pub trait Loadable: Decode<()> + Encode {
	const VERSION: u16;

	fn load(mut reader: impl Read) -> Result<Self, DecodeError> {
		fn bincode_error(error: IoError) -> DecodeError {
			DecodeError::Io {
				inner: error,
				additional: 0,
			}
		}

		let mut buf = vec![0; MAGIC.len()];
		reader.read_exact(&mut buf).map_err(bincode_error)?;

		if buf != MAGIC {
			return Err(DecodeError::Other("invalid config file"))
		}

		let mut buf = [0; 8];
		reader.read_exact(&mut buf).map_err(bincode_error)?;

		if buf[..2] != Self::VERSION.to_be_bytes() {
			return Err(DecodeError::Other("unsupported config version"))
		}

		let mut reader = DeflateDecoder::new(reader);
		bincode::decode_from_std_read(&mut reader, BINCODE_CONFIG)
	}

	fn save(&self, mut writer: impl Write) -> Result<(), EncodeError> {
		fn bincode_error(error: IoError) -> EncodeError {
			EncodeError::Io {
				inner: error,
				index: 0,
			}
		}

		writer.write_all(&MAGIC).map_err(bincode_error)?;
		writer
			.write_all(&Self::VERSION.to_be_bytes())
			.map_err(bincode_error)?;
		writer.write_all(&[0; 6]).map_err(bincode_error)?;

		let mut writer = DeflateEncoder::new(writer, Compression::best());
		bincode::encode_into_std_write(self, &mut writer, BINCODE_CONFIG)?;

		Ok(())
	}
}

/// A bundle of aerodrome configurations and maps.
#[derive(Clone, Debug, Decode, Encode)]
pub struct Config {
	pub name: Option<String>,
	pub version: Option<String>,

	/// Aerodrome configurations, listed with corresponding aerodrome ICAO.
	pub aerodromes: Vec<(String, Aerodrome)>,
	/// Maps, listed with corresponding aerodrome ICAO.
	pub maps: Vec<(String, Maps)>,
}

impl Loadable for Config {
	const VERSION: u16 = 0x0003;
}

#[derive(Debug, Decode, Encode)]
pub struct Ref<T>(pub usize, PhantomData<T>);

impl<T> Clone for Ref<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T> Copy for Ref<T> {}

impl<T> Hash for Ref<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.hash(state);
	}
}

impl<T> PartialEq for Ref<T> {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}

impl<T> Eq for Ref<T> {}

impl<T> PartialOrd for Ref<T> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		self.0.partial_cmp(&other.0)
	}
}

impl<T> Ord for Ref<T> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.0.cmp(&other.0)
	}
}

impl<T> From<usize> for Ref<T> {
	fn from(from: usize) -> Self {
		Self(from, PhantomData)
	}
}

impl<T> From<Ref<T>> for usize {
	fn from(from: Ref<T>) -> Self {
		from.0
	}
}
