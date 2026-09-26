use sdk::{ContractAddress, Event};

// These are the established Groundhog token movement events. Re-exporting
// keeps their signatures and encoding recognizable to existing consumers.
pub use groundhog_erc20_if::{BurnEvent, MintEvent, TransferEvent};

/// Context for a forced burn. The accompanying canonical BurnEvent is the
/// only supply movement; do not count this event as a second destruction.
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
    pub new_admin: &'a ContractAddress,
}

#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct SetMinterEvent<'a> {
    #[topic]
    pub minter: &'a ContractAddress,
    pub new_minter: &'a ContractAddress,
}

impl ClawbackEvent<'_> {
    pub fn log(&self) {
        sdk::log_event(self);
    }
}

impl SetAuthorizedEvent<'_> {
    pub fn log(&self) {
        sdk::log_event(self);
    }
}

impl SetAdminEvent<'_> {
    pub fn log(&self) {
        sdk::log_event(self);
    }
}

impl SetMinterEvent<'_> {
    pub fn log(&self) {
        sdk::log_event(self);
    }
}
