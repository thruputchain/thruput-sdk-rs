//! Operation names and fields follow SEP-41 and SAC, in Groundhog encoding.
//! Approve carries an absolute allowance and expiry, never a signed delta.
//! Allowance consumption emits only its transfer or burn event.

use sdk::{ContractAddress, Event};

// These are the established Groundhog token movement events. Re-exporting
// keeps their signatures and encoding recognizable to existing consumers.
pub use groundhog_erc20_if::{BurnEvent, MintEvent, TransferEvent};

#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct ApproveEvent<'a> {
    #[topic]
    pub from: &'a ContractAddress,
    #[topic]
    pub spender: &'a ContractAddress,
    pub amount: i128,
    pub live_until_ledger: u32,
}

#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct ClawbackEvent<'a> {
    #[topic]
    pub from: &'a ContractAddress,
    pub amount: i128,
}

#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct SetAuthorizedEvent<'a> {
    #[topic]
    pub id: &'a ContractAddress,
    pub authorize: bool,
}

#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct SetAdminEvent<'a> {
    #[topic]
    pub admin: &'a ContractAddress,
    pub new_admin: ContractAddress,
}

#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct SetMinterEvent<'a> {
    #[topic]
    pub minter: &'a ContractAddress,
    pub new_minter: ContractAddress,
}
