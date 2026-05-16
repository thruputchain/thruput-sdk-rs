#![cfg_attr(not(feature = "dev"), no_std)]

mod strings;
pub use strings::*;

use sdk::*;

#[sdk::contract]
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

    /// The token's decimal-places
    ///
    /// This parameter is to be used purely for presentation; it does not
    /// affect the represenation of any token amounts in the ERC20 contract.
    ///
    fn decimals(&self) -> Option<u32>;
}
