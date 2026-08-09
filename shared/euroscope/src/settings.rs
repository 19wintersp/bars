use std::ffi::{CStr, CString};
use std::str::FromStr;

pub trait SettingStore {
	fn save_raw_setting(&mut self, key: &CStr, value: &CStr, description: &CStr);
	fn load_raw_setting(&mut self, key: &CStr) -> Option<&CStr>;

	fn save_setting(
		&mut self,
		key: impl AsRef<[u8]>,
		value: &(impl SettingValue + ?Sized),
		description: &CStr,
	) {
		self.save_raw_setting(&encode(key.as_ref()), &value.encode(), description);
	}

	fn load_setting<T: SettingValue + ?Sized>(
		&mut self,
		key: impl AsRef<[u8]>,
	) -> Option<<T as ToOwned>::Owned> {
		self.load_raw_setting(&encode(key.as_ref())).and_then(T::decode)
	}
}

pub trait SettingValue: ToOwned {
	fn encode(&self) -> CString;
	fn decode(from: &CStr) -> Option<<Self as ToOwned>::Owned>;
}

impl<T: ToOwned + ToString + ?Sized> SettingValue for T
where
	<T as ToOwned>::Owned: FromStr,
{
	fn encode(&self) -> CString {
		encode(self.to_string().as_bytes())
	}

	fn decode(from: &CStr) -> Option<<Self as ToOwned>::Owned> {
		String::from_utf8(decode(from)).ok()?.parse().ok()
	}
}

const ESCAPE_ZERO: u8 = 0x3c;
static ALPHABET: &[u8; 64] =
	b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode(data: &[u8]) -> CString {
	let mut buf = Vec::with_capacity(data.len() + 1);

	for byte in data {
		if (0x40..0x7f).contains(byte) || (0x20..0x3a).contains(byte) {
			buf.push(*byte);
		} else {
			buf.push(ESCAPE_ZERO + (byte & 0b11));
			buf.push(ALPHABET[(byte >> 2) as usize]);
		}
	}

	buf.push(0);

	unsafe { CString::from_vec_with_nul_unchecked(buf) }
}

fn decode(data: &CStr) -> Vec<u8> {
	let mut buf = Vec::new();
	let mut escape = None;

	for byte in data.to_bytes() {
		if let Some(low_bits) = escape.take() {
			let Some(high_bits) = ALPHABET.iter().position(|symbol| symbol == byte)
			else {
				continue
			};

			buf.push(low_bits | (high_bits << 2) as u8);
		} else if (ESCAPE_ZERO..ESCAPE_ZERO + 4).contains(byte) {
			escape = Some(byte - ESCAPE_ZERO);
		} else {
			buf.push(*byte);
		}
	}

	buf
}
