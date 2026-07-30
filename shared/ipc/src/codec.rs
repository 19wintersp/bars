use std::io::{Error, ErrorKind};
use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio_util::bytes::{BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

const MAX_BUF_SIZE: usize = 0x100_0000;

pub struct Codec<T>(PhantomData<T>);

impl<T> Codec<T> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<T: DeserializeOwned> Decoder for Codec<T> {
	type Item = T;
	type Error = Error;

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<T>, Error> {
		let len = {
			if src.len() < 4 {
				return Ok(None)
			}

			let mut length = [0u8; 4];
			length.copy_from_slice(&src[..4]);
			u32::from_be_bytes(length) as usize
		};

		if len > MAX_BUF_SIZE {
			return Err(ErrorKind::FileTooLarge.into())
		}

		if src.len() >= len + 4 {
			let _ = src.split_to(4);
			let data = src.split_to(len);

			Ok(Some(
				postcard::from_bytes(&data)
					.map_err(|err| Error::new(ErrorKind::InvalidData, err))?,
			))
		} else {
			Ok(None)
		}
	}
}

impl<T: Serialize> Encoder<&T> for Codec<T> {
	type Error = Error;

	fn encode(&mut self, item: &T, dst: &mut BytesMut) -> Result<(), Error> {
		let data = postcard::to_stdvec(item)
			.map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;

		dst.put_u32(data.len() as u32);
		dst.put(data.as_slice());

		Ok(())
	}
}

impl<T: Serialize> Encoder<T> for Codec<T> {
	type Error = Error;

	fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Error> {
		self.encode(&item, dst)
	}
}
