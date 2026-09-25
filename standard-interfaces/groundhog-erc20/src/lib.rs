#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

mod error;
pub use error::*;

mod events;
pub use events::*;

use sdk::{ContractAddress, contract};

pub type Amount = u128;
pub type Difference = i128;

/// The all-zero address.
///
/// Nothing in this interface treats it specially any more: mints and burns
/// are [`MintEvent`] and [`BurnEvent`], not transfers from or to it. It
/// stays exported for consumers that render or model those events, and
/// as the address groundhogchain/hogd#3871 proposes reserving as "nothing
/// is here".
pub const CONTRACT_ADDR_ZERO: &ContractAddress =
    &ContractAddress::new([0; 32]);

/// The standard ERC20 interface.
///
/// # Error contract
///
/// These operations use the following [`ERC20Error`]s. Optional policy
/// extensions may reject an operation with additional errors, as with
/// `ERC20Pausable` and `ERC20Authorization`. Clients expose those through
/// their unknown-error path.
///
/// The basic reference uses one administrator for mint and burn. Issuer
/// extensions may delegate mint authority and restrict administrator burns:
/// possession of the role does not bypass the token's policy.
///
/// - [`ERC20Error::AlreadyInitialized`] — `initialize` called twice.
/// - [`ERC20Error::MustBeContractOwner`] — `mint`/`burn` called by anyone
///   other than the administrator.
/// - [`ERC20Error::TooLarge`] — an operation would drive a balance or
///   allowance below zero (overdraft), or an `Amount` doesn't fit in
///   [`Difference`].
#[contract(
    client = ERC20Client,
    throws = ERC20Error,
    emits(TransferEvent, ApprovalEvent, MintEvent, BurnEvent)
)]
pub trait ERC20 {
    /// Set the administrator. Throws [`ERC20Error::AlreadyInitialized`] if
    /// called more than once.
    fn initialize(&mut self, administrator: &ContractAddress);

    /// Transfer from caller to `receiver` address.
    /// Throws [`ERC20Error::TooLarge`] if the caller's balance is
    /// insufficient.
    fn transfer(&mut self, receiver: &ContractAddress, amount: Amount);

    /// Transfer from `owner` to `receiver` address, using allowance.
    /// Throws [`ERC20Error::TooLarge`] if the caller's allowance or the
    /// owner's balance is insufficient.
    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    );

    /// Replace the caller's remaining allowance for `spender` and emit
    /// [`ApprovalEvent`]. Zero revokes. Amounts above `i128::MAX` throw
    /// [`ERC20Error::TooLarge`]. Replacement must conflict with concurrent
    /// writes to the same allowance, including spending it.
    ///
    /// A spender may consume the old grant before a replacement executes.
    /// Use a one-shot signed transfer when spending itself must expire.
    fn approve(&mut self, spender: &ContractAddress, amount: Amount);

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

    /// Create tokens into `receiver` address and emit [`MintEvent`].
    /// Throws [`ERC20Error::MustBeContractOwner`] unless the caller is the
    /// administrator.
    fn mint(&mut self, receiver: &ContractAddress, amount: Amount);

    /// Destroy tokens from `owner` address and emit [`BurnEvent`].
    /// Throws [`ERC20Error::MustBeContractOwner`] unless the caller is the
    /// administrator, and [`ERC20Error::TooLarge`] if `owner`'s balance is
    /// less than `amount`.
    fn burn(&mut self, owner: &ContractAddress, amount: Amount);

    /// Get total supply of outstanding tokens
    fn total_supply(&self) -> Amount;
}
