#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! Optional controls for the standard Groundhog ERC20 interface.
//! These extensions support Stellar issuer policy without introducing a second
//! transfer or allowance interface. They do not implement Soroban's ABI or
//! the identity registry and compliance system described by ERC-3643.

mod error;
mod events;
pub use error::*;
pub use events::*;
use groundhog_erc20_if::{Amount, ERC20Error};
pub use groundhog_erc20_metadata_if::{Name, Symbol};
use sdk::{ContractAddress, contract, contract_type};

/// Immutable policy. Clawback requires revocation. SAC trustline history and
/// cross-chain policy synchronization are the bridge's responsibility.
#[contract_type]
pub struct Policy {
    pub auth_required: bool,
    pub auth_revocable: bool,
    pub clawback_enabled: bool,
}

/// Configuration of the issuer reference. Deploy and initialize atomically.
/// The caller becomes admin and may replace the minter, including with itself.
#[contract(client = TokenAdminClient, throws(TokenError, ERC20Error), emits(SetAdminEvent, SetMinterEvent))]
pub trait TokenAdmin {
    fn initialize(
        &mut self,
        minter: &ContractAddress,
        name: Name,
        symbol: Symbol,
        decimals: u32,
        policy: Policy,
    );
    fn admin(&self) -> ContractAddress;
    fn set_admin(&mut self, new_admin: &ContractAddress);
    fn minter(&self) -> ContractAddress;
    fn set_minter(&mut self, new_minter: &ContractAddress);
    fn policy(&self) -> Policy;
}

/// Optional holder authorization. Frozen holders cannot send, receive or burn.
/// Approvals and revocations remain available. A committed policy change is
/// visible to other transactions in the next block.
#[contract(client = TokenAuthorizationClient, throws = TokenError, emits(SetAuthorizedEvent))]
pub trait TokenAuthorization {
    fn authorized(&self, id: &ContractAddress) -> bool;
    /// Admin only. Setting false requires revocation enabled.
    fn set_authorized(&mut self, id: &ContractAddress, authorize: bool);
}

/// Optional forced destruction. Both this operation and ERC20's administrator
/// `burn` must honor the same immutable clawback policy. Frozen balances may
/// be clawed back. Emit one BurnEvent and one contextual ClawbackEvent.
#[contract(client = TokenClawbackClient, throws(TokenError, ERC20Error), emits(BurnEvent, ClawbackEvent))]
pub trait TokenClawback {
    fn clawback(&mut self, from: &ContractAddress, amount: Amount);
}

/// Optional holder-authorized destruction. Neither operation creates a bridge
/// withdrawal claim; the bridge must authenticate and record that separately.
#[contract(client = TokenBurnClient, throws(TokenError, ERC20Error), emits(BurnEvent))]
pub trait TokenBurn {
    fn burn(&mut self, amount: Amount);
    /// Consume the caller's ERC20 allowance from `owner`.
    fn burn_from(&mut self, owner: &ContractAddress, amount: Amount);
}
