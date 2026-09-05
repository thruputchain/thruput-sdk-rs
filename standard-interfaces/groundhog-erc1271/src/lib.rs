#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! ERC-1271-shaped signature validation: an opaque message, an opaque
//! signature, and the auth tokens the wallet chooses to mint.
//!
//! Superseded by `groundhog-signed-op-if`, which is the same idea with the
//! message format built in — typed, domain-separated, and replay-protected
//! with the chain's own uniquifier model — so that every wallet and every
//! client agree on what gets signed. New wallets should implement `SignedOp`.
//! This crate remains while contracts that implement it are migrated.

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
    /// The caller uses [`sdk::AssumeTokens::assume_tokens`] (or the fallible
    /// [`sdk::AssumeTokens::try_assume_tokens`]) to destructure the returned
    /// tokens into concrete `Signed<T>` values; a wrong token count is the
    /// same cross-call error as a return-decode failure.
    fn is_valid_signature(
        &self,
        message: &[u8],
        signature: &[u8],
    ) -> Vec<Signed<Opaque>>;
}
