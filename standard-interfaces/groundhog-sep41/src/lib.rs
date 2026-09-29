#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! SEP-0041 token semantics over Groundhog's ABI and caller authorization.
//! This is not the Soroban ABI: addresses are Groundhog contract addresses,
//! muxed addresses are unsupported, and events use Groundhog encoding.
//! The reference is a separate token profile with expiring grants. It reuses
//! native ERC20 balance and issuer-policy code, not its allowance entrypoints.
//! Cross-chain authorization and expiry synchronization are outside this interface.

mod error;
mod events;
pub use error::*;
pub use events::*;
use groundhog_erc20_if::{BurnEvent, ERC20Error, TransferEvent};
pub use groundhog_erc20_metadata_if::{Name, Symbol};
use sdk::{ContractAddress, contract};

/// Amounts must be non-negative. Overdrafts throw [`ERC20Error::TooLarge`].
/// Issuer controls may reject movement through the client's unknown-error path.
#[contract(client = SEP41Client, throws(SEP41Error, ERC20Error), emits(TransferEvent, BurnEvent, SEP41ApprovalEvent))]
pub trait SEP41 {
    /// Remaining grant, or zero after expiry. Expiry is inclusive and measured
    /// against Groundhog block height, not Stellar ledger sequence.
    fn allowance(
        &self,
        from: &ContractAddress,
        spender: &ContractAddress,
    ) -> i128;

    /// Replace the grant; the caller must equal `from`. A nonzero amount must
    /// expire at or after the current block. Zero revokes even with a past expiry.
    /// Replacement conflicts with concurrent writes to this grant. A spender
    /// may still consume the old grant before the replacement executes.
    fn approve(
        &mut self,
        from: &ContractAddress,
        spender: &ContractAddress,
        amount: i128,
        live_until_ledger: u32,
    );

    fn balance(&self, id: &ContractAddress) -> i128;

    /// The caller must equal `from`.
    fn transfer(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    );

    /// Spend a SEP-41 grant. The caller must equal `spender`; concurrent spending
    /// may merge within the allowance and balance bounds.
    fn transfer_from(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    );

    /// The caller must equal `from`. A burn alone is not a bridge withdrawal.
    fn burn(&mut self, from: &ContractAddress, amount: i128);

    /// Spend a SEP-41 grant; the caller must equal `spender`.
    fn burn_from(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        amount: i128,
    );

    /// Metadata queries throw [`SEP41Error::MetadataUnavailable`] when unset.
    fn decimals(&self) -> u32;
    fn name(&self) -> Name;
    fn symbol(&self) -> Symbol;
}
