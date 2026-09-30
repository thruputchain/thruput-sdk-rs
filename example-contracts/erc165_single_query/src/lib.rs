//! A contract whose ERC-165 predates `supports_interfaces`.
//!
//! [`SingleQueryERC165`] declares the same interface name and major version
//! as `groundhog_erc165_if::ERC165`, so it has the same interface id and the
//! same `supports_interface` entrypoint, but not the list query. That is
//! what a contract built against the single-query interface exports, and
//! the batch probes must still read its per-id answers rather than treat it
//! as having no ERC-165 at all.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;

/// ERC-165 with only the single-id query.
#[contract(
    interface = "ERC165",
    version = "0.1.0",
    client = SingleQueryERC165Client
)]
pub trait SingleQueryERC165 {
    /// True if this contract implements the interface identified by
    /// `interface_id`.
    fn supports_interface(interface_id: InterfaceId) -> bool;
}

/// Stateless carrier for the single-query ERC-165 impl.
pub struct SingleQueryContract;

#[contract]
impl SingleQueryERC165 for SingleQueryContract {
    fn supports_interface(interface_id: InterfaceId) -> bool {
        implements!(interface_id)
    }
}
