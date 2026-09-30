//! ERC-165 for Groundhog: a contract answers "do you implement interface
//! X?" for an [`InterfaceId`].
//!
//! On Ethereum an interface id is the XOR of the interface's function
//! selectors and `supportsInterface` is a hand-maintained claim. Here every
//! `#[contract]` trait already carries an `INTERFACE_ID` derived from its
//! name and major version — the pair that namespaces its entrypoint IDs —
//! and every `#[contract]` impl block leaves a record of that id in the
//! contract's interface table (see `sdk::interface_table`). So the claim
//! is not written at all: [`implements!`] reads it off the impls that
//! actually linked.
//!
//! ```ignore
//! groundhog_erc165_if::impl_erc165!();
//! ```
//!
//! That expands to a `#[contract] impl ERC165` on a hidden stateless
//! struct whose `supports_interface` is `implements!(id)`; the method
//! takes no `self`, so no storage struct is involved. Contracts that do
//! not invoke it carry no `supports_interface` entrypoint and no records
//! in their image: the table is anchored only by the reader behind
//! [`implements!`], so it is dropped from any image that never calls it.
//!
//! The Ethereum detection dance (a 30k-gas `staticcall`, then probing
//! `0xffffffff`) is unnecessary: a contract without this interface throws
//! the typed `LibraryExn::UnknownMethod`, which [`try_probe_interface`]
//! folds into `Ok(false)`, and there is no fallback function to lie.
//!
//! The claim remains self-reported in the sense that it describes this
//! contract's own code: `supports_interface` says "I export these
//! entrypoints under this interface's spec", nothing more. Treat it as a
//! dispatch hint, not an authorization boundary.

#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

use sdk::call::CallError;
use sdk::exn::{LibraryExn, MatchExn};
use sdk::{ContractAddress, contract};

pub use sdk::InterfaceId;
pub use sdk::implements;

/// The ERC-165 interface: query whether a contract implements an interface.
///
/// `supports_interface` returns `true` for [`ERC165::INTERFACE_ID`] itself,
/// since the impl block that provides it is one more record in the table.
/// It takes no `self`: the answer is a property of the contract's code, so
/// it neither loads nor touches storage, callers may invoke it read-only,
/// and the impl can live on any type — [`impl_erc165!`] puts it on a
/// hidden stateless one.
#[contract(client = ERC165Client)]
pub trait ERC165 {
    /// True if this contract implements the interface identified by
    /// `interface_id`.
    fn supports_interface(interface_id: InterfaceId) -> bool;
}

/// Give this contract ERC-165: a `#[contract] impl ERC165` whose
/// `supports_interface` is [`implements!`], on a hidden stateless carrier
/// struct.
///
/// ```ignore
/// groundhog_erc165_if::impl_erc165!();
/// ```
///
/// Invoke it once, at module level in the contract crate, next to the
/// contract's other `#[contract]` impls. `supports_interface` takes no
/// `self`, so the impl does not need — and is not tied to — the contract's
/// storage struct; the carrier is stateless and registers with the host
/// mock like a free function. Writing the same block by hand on any type
/// is equivalent.
#[macro_export]
macro_rules! impl_erc165 {
    () => {
        /// Stateless carrier for this contract's ERC-165 impl.
        #[doc(hidden)]
        pub struct __GroundhogErc165;

        #[::sdk::contract]
        impl $crate::ERC165 for __GroundhogErc165 {
            fn supports_interface(interface_id: $crate::InterfaceId) -> bool {
                $crate::implements!(interface_id)
            }
        }
    };
}

/// Ask `target` whether it implements `interface_id`, treating a contract
/// without ERC-165 as `Ok(false)`.
///
/// The query is a read-only call to `supports_interface`. A target that
/// does not export that entrypoint throws `LibraryExn::UnknownMethod`,
/// which is folded into `Ok(false)`; every other failure (no contract at
/// the address, a fault, an unrelated exception) is returned as the
/// [`CallError`] so a caller can distinguish "says no" from "cannot
/// answer".
pub fn try_probe_interface(
    target: &ContractAddress,
    interface_id: InterfaceId,
) -> Result<bool, CallError> {
    match ERC165Client::new(target)
        .with_read_only()
        .try_supports_interface(interface_id)
    {
        Ok(supported) => Ok(supported),
        Err(err) if err.try_as() == Some(LibraryExn::UnknownMethod) => {
            Ok(false)
        }
        Err(err) => Err(err),
    }
}

/// [`try_probe_interface`], re-throwing any failure other than a missing
/// `supports_interface` entrypoint.
pub fn probe_interface(
    target: &ContractAddress,
    interface_id: InterfaceId,
) -> bool {
    use sdk::OrReThrow;

    try_probe_interface(target, interface_id).or_re_throw()
}
