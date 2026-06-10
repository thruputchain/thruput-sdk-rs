#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

mod error;
pub use error::*;

mod events;
pub use events::*;

use sdk::{ContractAddress, contract};

pub type Amount = u128;
pub type Difference = i128;

pub const CONTRACT_ADDR_ZERO: &ContractAddress =
    &ContractAddress::new([0; 32]);

#[contract(
    client = ERC20Client,
    throws = ERC20Error,
    emits(TransferEvent, ApprovalEvent)
)]
pub trait ERC20 {
    fn initialize(&mut self, administrator: &ContractAddress);

    /// Transfer from caller to `receiver` address
    fn transfer(&mut self, receiver: &ContractAddress, amount: Amount);

    /// Transfer from `owner` to `receiver` address, using allowance
    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    );

    /// Adjust allowance for `spender` by `amount` (positive or negative)
    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        diff: Difference,
    );

    /// Get `spender`'s allowance of `owner`'s tokens
    fn allowance(
        &self,
        owner: &ContractAddress,
        spender: &ContractAddress,
    ) -> Amount;

    /// Get balance for given address
    fn get_balance_of(&self, addr: &ContractAddress) -> Amount;

    /// Get balance for caller
    fn get_balance(&self) -> Amount;

    /// Create tokens into `receiver` address
    fn mint(&mut self, receiver: &ContractAddress, amount: Amount);

    /// Destroy tokens from `owner` address
    fn burn(&mut self, owner: &ContractAddress, amount: Amount);

    /// Get total supply of outstanding tokens
    fn total_supply(&self) -> Amount;
}
