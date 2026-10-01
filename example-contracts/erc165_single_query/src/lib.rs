//! Stands in for a contract built against the older ERC-165: it exports
//! `supports_interface` but not `supports_interfaces`.

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
