//! ERC-165 for Groundhog: a contract answers "do you implement interface
//! X?" for an [`InterfaceId`].
//!
//! On Ethereum an interface id is the XOR of the interface's function
//! selectors and `supportsInterface` is a hand-maintained claim. Here every
//! `#[contract]` trait already carries an `INTERFACE_ID` derived from its
//! name and major version — the pair that namespaces its entrypoint IDs —
//! so the claim can be *derived* from the contract's trait impls and
//! checked by the compiler: [`impl_erc165!`] expands to `<T as Trait>::
//! INTERFACE_ID` for each listed trait, which does not compile unless `T`
//! implements it. Nothing is generated for contracts that do not invoke
//! the macro, so opting out costs no code size.
//!
//! The Ethereum detection dance (a 30k-gas `staticcall`, then probing
//! `0xffffffff`) is unnecessary: a contract without this interface throws
//! the typed `LibraryExn::UnknownMethod`, which [`try_probe_interface`]
//! folds into `Ok(false)`, and there is no fallback function to lie.
//!
//! The claim remains self-reported: `supports_interface` says "I export
//! these entrypoints under this interface's spec", nothing more. Treat it
//! as a dispatch hint, not an authorization boundary.

#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

use sdk::call::CallError;
use sdk::exn::{LibraryExn, MatchExn};
use sdk::{ContractAddress, contract};

pub use sdk::InterfaceId;

/// The ERC-165 interface: query whether a contract implements an interface.
///
/// `supports_interface` must return `true` for [`ERC165::INTERFACE_ID`]
/// itself. It takes no `self`: the answer is a property of the contract's
/// code, so it neither loads nor touches storage, and callers may invoke
/// it read-only.
///
/// Derive an implementation with [`impl_erc165!`] rather than writing one
/// by hand, so the claim stays tied to the traits the contract actually
/// implements.
#[contract(client = ERC165Client)]
pub trait ERC165 {
    /// True if this contract implements the interface identified by
    /// `interface_id`.
    fn supports_interface(interface_id: &InterfaceId) -> bool;
}

/// Derive `#[contract] impl ERC165 for $ty` from the traits `$ty`
/// implements.
///
/// ```ignore
/// groundhog_erc165_if::impl_erc165!(TokenContract: ERC20, ERC20Metadata);
/// ```
///
/// The generated `supports_interface` answers `true` for
/// [`ERC165::INTERFACE_ID`] and for `<$ty as Trait>::INTERFACE_ID` of every
/// listed trait. Each of those expressions compiles only if `$ty` actually
/// implements the trait, so a claim for an unimplemented interface is a
/// compile error, not a lie on chain. Traits are named by path, as in a
/// `use`; the list may be empty for a contract that supports only ERC-165.
///
/// This is the only `#[contract] impl ERC165` a contract should carry —
/// invoke it once, next to the contract's other `#[contract]` impls.
#[macro_export]
macro_rules! impl_erc165 {
    ($ty:ty : $($iface:path),* $(,)?) => {
        // The version pins this impl to the interface crate's major line
        // (like every impl of a standard interface), not to the invoking
        // crate's version. Bump it with the crate's major version; the
        // interface-version mismatch diagnostic catches a stale value at
        // the first `impl_erc165!` call site.
        #[::sdk::contract(version = "0.1.0")]
        impl $crate::ERC165 for $ty {
            fn supports_interface(interface_id: &$crate::InterfaceId) -> bool {
                *interface_id == <$ty as $crate::ERC165>::INTERFACE_ID
                    $(|| *interface_id == <$ty as $iface>::INTERFACE_ID)*
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
    interface_id: &InterfaceId,
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
    interface_id: &InterfaceId,
) -> bool {
    use sdk::OrReThrow;

    try_probe_interface(target, interface_id).or_re_throw()
}
