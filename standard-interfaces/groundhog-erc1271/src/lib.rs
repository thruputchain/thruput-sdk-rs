#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

extern crate alloc;

mod error;
pub use error::*;

use alloc::vec::Vec;
use sdk::contract;
use sdk::signer::{Opaque, Signed};

#[contract(client = ERC1271Client, throws = ERC1271Error)]
pub trait ERC1271 {
    /// Validates the given signature against the given message and returns
    /// any auth tokens the wallet wishes to produce.
    ///
    /// Returns an empty Vec if the wallet authorizes but no tokens are needed.
    /// Aborts (throws) if the wallet refuses to authorize.
    ///
    /// The caller uses [`AssumeTokens::assume_tokens`] to destructure the
    /// returned tokens into concrete `Signed<T>` values.
    fn is_valid_signature(
        &self,
        message: &[u8],
        signature: &[u8],
    ) -> Vec<Signed<Opaque>>;
}
