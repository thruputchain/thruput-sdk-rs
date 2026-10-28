//! The ERC20 metadata extension for Thruput smart contracts: a token's
//! name, symbol and decimal places.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use thruput_sdk::*;

pub type Name = heapless::String<100>;
/// Includes the 12-character codes of classic Stellar assets.
pub type Symbol = heapless::String<12>;

#[thruput_sdk::contract]
pub trait ERC20Metadata {
    fn initialize(
        &mut self,
        administrator: &ContractAddress,
        name: Option<Name>,
        symbol: Option<Symbol>,
        decimals: Option<u32>,
    );

    /// The token's name
    fn name(&self) -> Option<Name>;

    /// The token's symbol
    fn symbol(&self) -> Option<Symbol>;

    /// The token's number of decimal places.
    ///
    /// This is for presentation only; it does not affect how token amounts
    /// are represented in the ERC20 contract.
    fn decimals(&self) -> Option<u32>;
}
