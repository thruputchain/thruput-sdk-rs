#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

mod error;
pub use error::*;

mod events;
pub use events::*;

use sdk::{BlockHeight, ContractAddress, contract};

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
/// The basic reference uses one administrator for mint. Issuer extensions may
/// delegate mint authority. Destruction lives in extensions: holder burns in
/// `ERC20Burn`, forced destruction in `ERC20Clawback`.
///
/// - [`ERC20Error::AlreadyInitialized`] — `initialize` called twice.
/// - [`ERC20Error::MustBeContractOwner`] — `mint` called by anyone other than
///   the administrator.
/// - [`ERC20Error::TooLarge`] — an operation would drive a balance or
///   allowance below zero (overdraft), spend an expired allowance, or an
///   `Amount` doesn't fit in [`Difference`].
/// - [`ERC20Error::BalanceNotZero`] — `clear_balance` called with a nonzero
///   balance.
#[contract(
    client = ERC20Client,
    throws = ERC20Error,
    emits(TransferEvent, AllowanceChangeEvent, MintEvent)
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
    /// Throws [`ERC20Error::TooLarge`] if the caller's allowance is
    /// insufficient or expired, or the owner's balance is insufficient.
    fn transfer_from(
        &mut self,
        owner: &ContractAddress,
        receiver: &ContractAddress,
        amount: Amount,
    );

    /// Add `difference` to the caller's allowance for `spender` and emit
    /// [`AllowanceChangeEvent`]. Negative differences reduce the allowance.
    /// A result outside `0..=i128::MAX` throws [`ERC20Error::TooLarge`].
    /// The expiry is unchanged, so adjusting an expired allowance does not
    /// make it spendable again.
    ///
    /// Adjustments may merge with concurrent spending and adjustments within
    /// the allowance bounds. To set an absolute allowance, call
    /// `clear_allowance` first in the same transaction: its exclusive lock
    /// covers this write too.
    fn adjust_allowance(
        &mut self,
        spender: &ContractAddress,
        difference: Difference,
    );

    /// Set the last block, inclusive, at which `spender` may spend the
    /// caller's allowance, or `None` for no expiry. After it, `allowance`
    /// reads zero and spending throws [`ERC20Error::TooLarge`]; the amount
    /// stays stored until `clear_allowance`. An allowance without an expiry
    /// stores none. Emits no event.
    ///
    /// The write is exclusive. Transactions elsewhere in the same block may
    /// still spend under the previous expiry.
    fn set_allowance_expiry(
        &mut self,
        spender: &ContractAddress,
        until: Option<BlockHeight>,
    );

    /// Revoke the caller's allowance for `spender` and delete its storage entry,
    /// including any expiry. Missing entries are allowed. Deletion conflicts
    /// with concurrent writes, including grants and spending, and the lock
    /// holds for later writes in this transaction. Emit
    /// [`AllowanceChangeEvent`] with the negative remaining allowance (zero for
    /// a missing entry).
    /// Implementations SHOULD refund the released storage cost to the caller.
    fn clear_allowance(&mut self, spender: &ContractAddress);

    /// Delete the caller's empty balance entry, leaving supply unchanged.
    /// Throw [`ERC20Error::BalanceNotZero`] for a nonzero balance. Missing
    /// entries are allowed. Deletion conflicts with concurrent balance writes.
    /// Do not delete holder authorization or emit a token movement event.
    /// Implementations SHOULD refund the released storage cost to the caller.
    fn clear_balance(&mut self);

    /// Get `spender`'s allowance of `owner`'s tokens, or zero once expired
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

    /// Get total supply of outstanding tokens
    fn total_supply(&self) -> Amount;
}
