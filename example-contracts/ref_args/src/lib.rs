//! Contract methods that take their arguments by reference.
//!
//! Fixed-size types (`&ContractAddress`, `&[u8]`) are borrowed from the
//! calldata buffer. Bounded vectors (`&heapless::Vec`, `&VecM`) cannot be,
//! so they are decoded into a local that the method borrows.

#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

sdk::declare_mock_contract!();

use sdk::xdr::VecM;
use sdk::*;

/// A bounded byte list, decoded with no allocator.
pub type Bytes = heapless::Vec<u8, 16>;

/// A bounded word list, decoded into an allocated vector.
pub type Words = VecM<u32, 8>;

/// Stateless carrier for the by-reference methods.
pub struct RefArgsContract;

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
}

#[cfg(feature = "bottle-test")]
mod bottle_test;
