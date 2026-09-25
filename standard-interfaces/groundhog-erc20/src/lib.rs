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
/// - [`ERC20Error::BalanceNotZero`] — `clear_balance` called with a nonzero
///   balance.
#[contract(
    client = ERC20Client,
    throws = ERC20Error,
    emits(TransferEvent, AllowanceChangeEvent, ApprovalEvent, MintEvent, BurnEvent)
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

    /// Add `difference` to the caller's allowance for `spender` and emit
    /// [`AllowanceChangeEvent`]. Negative differences reduce the allowance.
    /// A result outside `0..=i128::MAX` throws [`ERC20Error::TooLarge`].
    ///
    /// With `exclusive=false`, adjustments may merge with concurrent spending
    /// and adjustments within the allowance bounds. With `exclusive=true`,
    /// this transaction conflicts with other writers to the same pair,
    /// including when `difference` is zero. Different pairs remain independent.
    fn increase_allowance(
        &mut self,
        spender: &ContractAddress,
        difference: Difference,
        exclusive: bool,
    );

    /// Revoke the caller's allowance for `spender` and delete its storage entry.
    /// Missing entries are allowed. Deletion conflicts with concurrent writes,
    /// including grants and spending. Emit [`ApprovalEvent`] with amount zero.
    fn clear_allowance(&mut self, spender: &ContractAddress);

    /// Delete the caller's empty balance entry, leaving supply unchanged.
    /// Throw [`ERC20Error::BalanceNotZero`] for a nonzero balance. Missing
    /// entries are allowed. Deletion conflicts with concurrent balance writes.
    /// Do not delete holder authorization or emit a token movement event.
    fn clear_balance(&mut self);

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

impl ERC20Client {
    /// Replace this calling contract's allowance using an exclusive adjustment.
    /// This is a library helper, not a token entrypoint. It reads the allowance
    /// then calls [`Self::increase_allowance`], which emits a delta event.
    /// Even an unchanged target makes an exclusive call.
    ///
    /// A spender may use the old allowance before this transaction executes.
    /// Stored allowances do not expire. For spending with a deadline, use
    /// `ERC20Permit::transfer_from_signed` with an expiring signed operation.
    pub fn approve(&self, spender: &ContractAddress, amount: Amount) {
        let target = amount_to_diff(amount);
        let current = amount_to_diff(
            self.allowance(&sdk::get_current_address(), spender),
        );
        self.increase_allowance(spender, target - current, true);
    }
}
