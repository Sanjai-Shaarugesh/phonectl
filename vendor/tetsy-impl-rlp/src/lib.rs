// Copyright 2015-2018 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! RLP serialization support for uint and fixed hash.

#![cfg_attr(not(feature = "std"), no_std)]

#[doc(hidden)]
pub extern crate tetsy_rlp;

#[doc(hidden)]
pub extern crate core as core_;

/// Add RLP serialization support to an integer created by `construct_uint!`.
#[macro_export]
macro_rules! impl_uint_rlp {
	($name: ident, $size: expr) => {
		impl $crate::tetsy_rlp::Encodable for $name {
			fn rlp_append(&self, s: &mut $crate::tetsy_rlp::RlpStream) {
				let leading_empty_bytes = $size * 8 - (self.bits() + 7) / 8;
				let mut buffer = [0u8; $size * 8];
				self.to_big_endian(&mut buffer);
				s.encoder().encode_value(&buffer[leading_empty_bytes..]);
			}
		}

		impl $crate::tetsy_rlp::Decodable for $name {
			fn decode(tetsy_rlp: &$crate::tetsy_rlp::Rlp) -> Result<Self, $crate::tetsy_rlp::DecoderError> {
				tetsy_rlp.decoder().decode_value(|bytes| {
					if !bytes.is_empty() && bytes[0] == 0 {
						Err($crate::tetsy_rlp::DecoderError::RlpInvalidIndirection)
					} else if bytes.len() <= $size * 8 {
						Ok($name::from(bytes))
					} else {
						Err($crate::tetsy_rlp::DecoderError::RlpIsTooBig)
					}
				})
			}
		}
	}
}

/// Add RLP serialization support to a fixed-sized hash type created by `construct_fixed_hash!`.
#[macro_export]
macro_rules! impl_fixed_hash_rlp {
	($name: ident, $size: expr) => {
		impl $crate::tetsy_rlp::Encodable for $name {
			fn rlp_append(&self, s: &mut $crate::tetsy_rlp::RlpStream) {
				s.encoder().encode_value(self.as_ref());
			}
		}

		impl $crate::tetsy_rlp::Decodable for $name {
			fn decode(tetsy_rlp: &$crate::tetsy_rlp::Rlp) -> Result<Self, $crate::tetsy_rlp::DecoderError> {
				tetsy_rlp.decoder().decode_value(|bytes| match bytes.len().cmp(&$size) {
					$crate::core_::cmp::Ordering::Less => Err($crate::tetsy_rlp::DecoderError::RlpIsTooShort),
					$crate::core_::cmp::Ordering::Greater => Err($crate::tetsy_rlp::DecoderError::RlpIsTooBig),
					$crate::core_::cmp::Ordering::Equal => {
						let mut t = [0u8; $size];
						t.copy_from_slice(bytes);
						Ok($name(t))
					}
				})
			}
		}
	}
}
