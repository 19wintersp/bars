mod aerodrome;
mod map;

use std::cmp::Ordering;
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::io::{Error as IoError, Read, Write};
use std::marker::PhantomData;
use std::num::NonZeroU8;
use std::str::FromStr;

use bincode::config::Configuration as BincodeConfig;
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Config {
	pub name: Option<String>,
	pub version: Option<String>,

	/// Aerodrome configurations, listed with corresponding aerodrome ICAO.
	pub aerodromes: Vec<(Icao, Aerodrome)>,
	/// Maps, listed with corresponding aerodrome ICAO.
	pub maps: Vec<(Icao, Maps)>,
}

impl Loadable for Config {
	const VERSION: u16 = 0x0003;
}

#[derive(Debug, Decode, Encode)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
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

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
	feature = "serde",
	derive(Deserialize, Serialize),
	serde(try_from = "String", into = "String")
)]
pub struct Icao([NonZeroU8; 4]);

impl Icao {
	pub fn try_new(s: &str) -> Result<Self, ParseIcaoError> {
		if s.as_bytes().len() != 4 {
			Err(ParseIcaoError::Length)
		} else if !s
			.chars()
			.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
		{
			Err(ParseIcaoError::Character)
		} else {
			let mut buf = [0; 4];
			buf.copy_from_slice(s.as_bytes());
			Ok(Self(unsafe { std::mem::transmute(buf) }))
		}
	}

	pub fn as_bytes(&self) -> &[u8; 4] {
		unsafe { std::mem::transmute(self) }
	}

	pub fn as_str(&self) -> &str {
		unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
	}
}

impl Debug for Icao {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}", self.as_str())
	}
}

impl Display for Icao {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl FromStr for Icao {
	type Err = ParseIcaoError;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::try_new(s)
	}
}

impl From<Icao> for String {
	fn from(from: Icao) -> Self {
		from.as_str().into()
	}
}

impl TryFrom<String> for Icao {
	type Error = ParseIcaoError;
	fn try_from(from: String) -> Result<Self, Self::Error> {
		Self::try_new(&from)
	}
}

impl<'de, Context> BorrowDecode<'de, Context> for Icao {
	fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
		decoder: &mut D,
	) -> Result<Self, DecodeError> {
		Self::decode(decoder)
	}
}

impl<Context> Decode<Context> for Icao {
	fn decode<D: Decoder<Context = Context>>(
		decoder: &mut D,
	) -> Result<Self, DecodeError> {
		String::decode(decoder)?
			.parse::<Self>()
			.map_err(|err| DecodeError::OtherString(err.to_string()))
	}
}

impl Encode for Icao {
	fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
		self.as_str().encode(encoder)
	}
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseIcaoError {
	Length,
	Character,
}

impl Display for ParseIcaoError {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		match self {
			Self::Length => write!(f, "invalid length"),
			Self::Character => write!(f, "invalid character"),
		}
	}
}

impl Error for ParseIcaoError {}
