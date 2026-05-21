#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use sdk::*;

pub type Name = heapless::String<100>;
pub type Symbol = heapless::String<10>;

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
