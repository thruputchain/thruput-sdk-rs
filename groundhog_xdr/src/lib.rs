#![cfg_attr(not(feature = "std"), no_std)]
#![allow(non_snake_case)]
#![allow(unused_imports)]

mod byte_array;
pub use byte_array::*;

#[rustfmt::skip]
pub mod contract;
#[rustfmt::skip]
pub mod syscall;
#[rustfmt::skip]
pub mod transaction;
#[rustfmt::skip]
pub mod types;
#[rustfmt::skip]
pub mod ffi_enums;

mod common_types;
pub use common_types::*;

#[cfg(feature = "std")]
mod serialize;
#[cfg(feature = "std")]
pub use serialize::*;

#[cfg(feature = "std")]
#[rustfmt::skip]
mod syscall_args;
#[cfg(feature = "std")]
pub use syscall_args::*;

use contract::*;
use ffi_enums::*;
use syscall::*;
use transaction::*;
use types::*;

#[cfg(test)]
mod test;

impl Default for Signature {
    fn default() -> Self {
        Self::Ed25519 {
            ed25519: ByteArray::default(),
        }
    }
}

impl Default for PublicKey {
    fn default() -> Self {
        Self::Ed25519 {
            ed25519: ByteArray::default(),
        }
    }
}

pub trait ToXdr<T> {
    fn to_xdr_struct(&self) -> T;
}

pub trait FromXdr<T> {
    fn from_xdr_struct(arg: &T) -> Self;
}

impl ToXdr<int128_xdr> for i128 {
    fn to_xdr_struct(&self) -> int128_xdr {
        return int128_xdr {
            highbits: (self >> 64) as i64,
            lowbits: (self & (u64::MAX as i128)) as u64,
        };
    }
}

impl FromXdr<int128_xdr> for i128 {
    fn from_xdr_struct(arg: &int128_xdr) -> Self {
        return ((arg.highbits as i128) << 64) + (arg.lowbits as i128);
    }
}
