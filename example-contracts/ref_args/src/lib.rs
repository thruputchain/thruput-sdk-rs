//! Contract methods that take their arguments by reference.
//!
//! Every kind of `&T` parameter the SDK accepts, so host tests, the rv32
//! build, and the bottle test cover each decode path:
//! - borrowed from the calldata buffer: `&[u8]`, `&str`, `&[u8; N]`, and the
//!   fixed-size protocol types; `borrowed_offsets` reports where each one
//!   points so the tests can check nothing was copied;
//! - decoded into a local: scalars, `Option`, tuples, arrays, bounded
//!   vectors and strings, and `#[contract_type]` types;
//! - slices: `&[T]` decoded into a local vector, with byte-string elements
//!   (`&[&[u8]]`, `&[&str]`) still borrowing their bytes;
//! - a vector of borrowed byte strings by value, `VecM<&[u8], N>`.

#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

sdk::declare_mock_contract!();

use sdk::xdr::{BytesM, VecM};
use sdk::*;

/// A bounded byte list, decoded with no allocator.
pub type Bytes = heapless::Vec<u8, 16>;

/// A bounded word list, decoded into an allocated vector.
pub type Words = VecM<u32, 8>;

/// A bounded string, decoded with no allocator.
pub type Label = heapless::String<16>;

/// A user-defined type taken by reference.
#[contract_type]
#[cfg_attr(feature = "dev", derive(Debug, Clone, PartialEq, Eq))]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

/// Stateless carrier for the by-reference methods.
pub struct RefArgsContract;

/// Offset of `ptr` from `base`, both pointing into one calldata buffer.
fn offset<T: ?Sized, U: ?Sized>(base: *const T, ptr: *const U) -> u32 {
    (ptr as *const u8 as usize).wrapping_sub(base as *const u8 as usize) as u32
}

#[contract(client = RefArgsClient)]
impl RefArgsContract {
    /// The sum of the bytes in `bytes`.
    pub fn sum_bytes(bytes: &Bytes) -> u32 {
        bytes.iter().map(|b| u32::from(*b)).sum()
    }

    /// The sum of the words in `words`.
    pub fn sum_words(words: &Words) -> u32 {
        words.0.iter().sum()
    }

    /// Owned, borrowed-from-calldata, and decoded-into-local parameters in
    /// one call: `tag`, plus the first byte of `owner`, plus the lengths of
    /// `bytes`, `words`, and `raw`.
    pub fn mixed(
        tag: u32,
        owner: &ContractAddress,
        bytes: &Bytes,
        words: &Words,
        raw: &[u8],
    ) -> u32 {
        let lens = bytes.len() + words.0.len() + raw.len();
        tag + u32::from(owner.as_ref()[0]) + lens as u32
    }

    /// Where each borrowed argument's bytes sit relative to `address`. All
    /// of them point into the calldata buffer, so the offsets are the
    /// positions of their bytes in the encoded arguments.
    pub fn borrowed_offsets(
        address: &ContractAddress,
        bytes: &[u8],
        text: &str,
        fixed: &[u8; 4],
    ) -> [u32; 3] {
        let base = address as *const ContractAddress;
        [offset(base, bytes), offset(base, text), offset(base, fixed)]
    }

    /// Where each borrowed argument sits relative to `hash`: the list
    /// elements' bytes stay in calldata even though the lists are built in
    /// locals.
    pub fn borrowed_list_offsets(
        hash: &SHA256Hash,
        strings: &[&[u8]],
        words: &[&str],
    ) -> [u32; 3] {
        let base = hash as *const SHA256Hash;
        [
            offset(base, strings[0]),
            offset(base, strings[1]),
            offset(base, words[0]),
        ]
    }

    /// Scalars by reference, decoded into locals: `a + b + c + d`.
    pub fn scalars(a: &u32, b: &i64, c: &bool, d: &u128) -> i128 {
        i128::from(*a) + i128::from(*b) + i128::from(*c) + *d as i128
    }

    /// Composites by reference: the sum of every number they carry, plus
    /// the first byte of each address and id.
    pub fn composites(
        maybe: &Option<u32>,
        pair: &(u32, ContractAddress),
        numbers: &[u32; 3],
        ids: &[[u8; 8]; 2],
        owners: &[ContractAddress; 2],
    ) -> u64 {
        let mut total = u64::from(maybe.unwrap_or(0)) + u64::from(pair.0);
        total += u64::from(pair.1.as_ref()[0]);
        total += numbers.iter().map(|n| u64::from(*n)).sum::<u64>();
        total += ids.iter().map(|id| u64::from(id[0])).sum::<u64>();
        total += owners.iter().map(|o| u64::from(o.as_ref()[0])).sum::<u64>();
        total
    }

    /// Bounded strings by reference: the total length.
    pub fn text_lengths(label: &Label, blob: &BytesM<16>) -> u32 {
        (label.len() + blob.len()) as u32
    }

    /// A `#[contract_type]` value and a slice of them: `x + y` summed.
    pub fn points(point: &Point, more: &[Point]) -> u32 {
        point.x + point.y + more.iter().map(|p| p.x + p.y).sum::<u32>()
    }

    /// Slices of non-byte elements, decoded into local vectors: the sum of
    /// `numbers` plus the first byte of each address.
    pub fn slices(numbers: &[u32], owners: &[ContractAddress]) -> u64 {
        numbers.iter().map(|n| u64::from(*n)).sum::<u64>()
            + owners.iter().map(|o| u64::from(o.as_ref()[0])).sum::<u64>()
    }

    /// A vector of borrowed byte strings, by value and by reference: the
    /// total length of both, and the offset of the second string of
    /// `strings` from its first, which is its position in calldata when
    /// both borrow from it.
    pub fn byte_string_lists(
        strings: VecM<&[u8], 4>,
        more: &VecM<&[u8], 4>,
    ) -> (u32, u32) {
        let total = strings.0.iter().chain(more.0.iter()).map(|s| s.len());
        (
            total.sum::<usize>() as u32,
            offset(strings.0[0], strings.0[1]),
        )
    }
}

#[cfg(feature = "bottle-test")]
mod bottle_test;

/// Where `borrowed_offsets` must find each argument's bytes if it borrowed
/// them from calldata: their positions in the encoded arguments. Each
/// argument carries marker bytes that appear nowhere else in the encoding.
#[cfg(any(feature = "dev", feature = "bottle-test"))]
pub mod expected {
    use super::*;

    pub const ADDRESS: ContractAddress = ContractAddress::new([0xA1; 32]);
    pub const BYTES: &[u8] = &[0xB1, 0xB2, 0xB3];
    pub const TEXT: &str = "qrs";
    pub const FIXED: [u8; 4] = [0xF1, 0xF2, 0xF3, 0xF4];
    pub const HASH: SHA256Hash = SHA256Hash::new([0xC7; 32]);
    pub const STRING_0: &[u8] = &[0xD1, 0xD1];
    pub const STRING_1: &[u8] = &[0xD2, 0xD2, 0xD2];
    pub const WORD: &str = "uvw";

    fn position(haystack: &[u8], needle: &[u8]) -> u32 {
        haystack
            .windows(needle.len())
            .position(|w| w == needle)
            .unwrap_or_else(|| panic!("marker bytes missing from encoding"))
            as u32
    }

    /// The offsets `borrowed_offsets` returns when every argument points
    /// into the calldata the client sent.
    pub fn borrowed_offsets() -> [u32; 3] {
        let encoded = sdk::encode_xdr(&(&ADDRESS, BYTES, TEXT, &FIXED))
            .unwrap_or_else(|_| panic!("arguments encode"));
        let base = position(&encoded, ADDRESS.as_ref());
        [
            position(&encoded, BYTES) - base,
            position(&encoded, TEXT.as_bytes()) - base,
            position(&encoded, &FIXED) - base,
        ]
    }

    /// The offsets `borrowed_list_offsets` returns when every argument
    /// points into the calldata the client sent.
    pub fn borrowed_list_offsets() -> [u32; 3] {
        let strings: &[&[u8]] = &[STRING_0, STRING_1];
        let words: &[&str] = &[WORD];
        let encoded = sdk::encode_xdr(&(&HASH, strings, words))
            .unwrap_or_else(|_| panic!("arguments encode"));
        let base = position(&encoded, HASH.as_ref());
        [
            position(&encoded, STRING_0) - base,
            position(&encoded, STRING_1) - base,
            position(&encoded, WORD.as_bytes()) - base,
        ]
    }
}
