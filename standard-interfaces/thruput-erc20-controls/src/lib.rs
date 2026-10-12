#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

//! Optional controls for the standard Thruput ERC20 interface.
//! These extensions support Stellar issuer policy without introducing a second
//! transfer or allowance interface. Authorization follows Thruput's caller
//! model, not Soroban's authorization tree.

mod error;
mod events;
pub use error::*;
pub use events::*;
use sdk::{ContractAddress, contract, contract_type};
use thruput_erc20_if::{Amount, ERC20Error};
pub use thruput_erc20_metadata_if::{Name, Symbol};

/// Policy fixed at initialization. `clawback_enabled` requires
/// `auth_revocable`. SAC trustline history and cross-chain policy
/// synchronization are the bridge's responsibility.
#[contract_type]
pub struct Policy {
    /// Holders start unauthorized when true, authorized when false.
    pub auth_required: bool,
    /// The admin may revoke a holder's authorization.
    pub auth_revocable: bool,
    /// The admin may destroy tokens without the holder's consent.
    pub clawback_enabled: bool,
}

/// Configuration of the issuer reference. Deploy and initialize atomically.
/// The caller becomes admin and may replace the minter, including with itself.
/// Role changes reach other transactions in the next block.
#[contract(client = ERC20AdminClient, throws(ERC20ControlsError, ERC20Error), emits(SetAdminEvent, SetMinterEvent))]
pub trait ERC20Admin {
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
/// Setting or revoking allowances remains available to frozen holders.
/// A committed `set_authorized` change is visible to other transactions in
/// the next block.
#[contract(client = ERC20AuthorizationClient, throws = ERC20ControlsError, emits(SetAuthorizedEvent))]
pub trait ERC20Authorization {
    fn authorized(&self, id: &ContractAddress) -> bool;
    /// Admin only. Setting `authorize` to false requires `auth_revocable`.
    fn set_authorized(&mut self, id: &ContractAddress, authorize: bool);
}

/// Optional forced destruction. Both this operation and ERC20's administrator
/// `burn` must honor the same immutable clawback policy. Frozen balances may
/// be clawed back. Emit one BurnEvent and one contextual ClawbackEvent.
#[contract(client = ERC20ClawbackClient, throws(ERC20ControlsError, ERC20Error), emits(BurnEvent, ClawbackEvent))]
pub trait ERC20Clawback {
    fn clawback(&mut self, from: &ContractAddress, amount: Amount);
}

/// Optional holder-authorized destruction. Neither operation creates a bridge
/// withdrawal claim; the bridge must authenticate and record that separately.
#[contract(client = ERC20BurnClient, throws(ERC20ControlsError, ERC20Error), emits(BurnEvent))]
pub trait ERC20Burn {
    fn burn(&mut self, amount: Amount);
    /// Burn `amount` from `owner` and decrease the caller's allowance by it.
    fn burn_from(&mut self, owner: &ContractAddress, amount: Amount);
}
