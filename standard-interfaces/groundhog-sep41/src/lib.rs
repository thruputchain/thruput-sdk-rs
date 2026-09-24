#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! A Groundhog adaptation of SEP-41, with separate issuer controls.
//!
//! Calls use Groundhog addresses, caller authorization and CXS encoding.
//! `live_until_ledger` is a Groundhog block height, inclusive. Events use
//! Groundhog signatures and encoding, not Soroban's wire representation.
//! See `docs/sep41-reference-token.md` for the compatibility boundary.

mod error;
mod events;
pub use error::*;
pub use events::*;

use sdk::{ContractAddress, contract, contract_type};

pub type Name = heapless::String<100>;
/// Covers Stellar asset codes, including 12-character classic issued assets.
pub type Symbol = heapless::String<12>;

/// Immutable policy chosen when a reference token is initialized.
/// Clawback requires revocation. This does not model SAC trustline history.
#[contract_type]
pub struct Policy {
    pub auth_required: bool,
    pub auth_revocable: bool,
    pub clawback_enabled: bool,
}

/// SEP-41 operations, adapted to Groundhog caller authorization.
/// All amounts must be non-negative. Mutations fail atomically.
#[contract(
    client = TokenClient,
    throws = TokenError,
    emits(TransferEvent, ApproveEvent, BurnEvent)
)]
pub trait Token {
    fn balance(&self, id: &ContractAddress) -> i128;
    /// Missing or expired allowance is zero.
    fn allowance(
        &self,
        from: &ContractAddress,
        spender: &ContractAddress,
    ) -> i128;
    /// Caller must equal `from`. Replace the remaining allowance and expiry.
    /// Zero revokes, including when the supplied expiry is in the past.
    /// Nonzero grants require expiry >= the executing Groundhog block height.
    /// Approvals remain allowed while frozen; spending still checks holders.
    /// Replacing a live grant retains the usual allowance replacement race.
    fn approve(
        &mut self,
        from: &ContractAddress,
        spender: &ContractAddress,
        amount: i128,
        live_until_ledger: u32,
    );
    /// Caller must equal `from`; both holders must be authorized.
    fn transfer(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    );
    /// Caller must equal `spender`. Consume allowance without extending expiry.
    /// Both holders must be authorized, even if allowance was granted earlier.
    fn transfer_from(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        to: &ContractAddress,
        amount: i128,
    );
    /// Caller must equal `from`, who must be authorized. Reduces supply.
    /// A burn is not a bridge withdrawal request.
    fn burn(&mut self, from: &ContractAddress, amount: i128);
    /// Caller must equal `spender`. Consumes allowance; `from` must be authorized.
    fn burn_from(
        &mut self,
        spender: &ContractAddress,
        from: &ContractAddress,
        amount: i128,
    );
    fn name(&self) -> Name;
    fn symbol(&self) -> Symbol;
    fn decimals(&self) -> u32;
}

/// Administration for the reference token; not part of core SEP-41.
#[contract(
    client = TokenAdminClient,
    throws = TokenError,
    emits(MintEvent, ClawbackEvent, SetAuthorizedEvent, SetAdminEvent, SetMinterEvent)
)]
pub trait TokenAdmin {
    /// Initialize once in the deployment transaction. Caller becomes admin.
    /// Minter may mint; admin may replace the minter as well as manage holders.
    fn initialize(
        &mut self,
        minter: &ContractAddress,
        name: Name,
        symbol: Symbol,
        decimals: u32,
        policy: Policy,
    );
    fn admin(&self) -> ContractAddress;
    /// Current admin only; updates are effective for other transactions next block.
    fn set_admin(&mut self, new_admin: &ContractAddress);
    fn minter(&self) -> ContractAddress;
    /// Admin only. Admin can appoint itself minter; it controls mint governance.
    /// Mint authority does not grant freeze or clawback authority.
    fn set_minter(&mut self, new_minter: &ContractAddress);
    /// Minter only. Recipient must be authorized.
    fn mint(&mut self, to: &ContractAddress, amount: i128);
    fn authorized(&self, id: &ContractAddress) -> bool;
    /// Admin only. Setting false requires revocation enabled.
    /// Other transactions observe a committed change in the next block.
    fn set_authorized(&mut self, id: &ContractAddress, authorize: bool);
    /// Admin only, with clawback enabled. May burn a frozen holder's balance.
    fn clawback(&mut self, from: &ContractAddress, amount: i128);
    /// Groundhog extension for accounting. Bounded to i128::MAX.
    fn total_supply(&self) -> i128;
}
