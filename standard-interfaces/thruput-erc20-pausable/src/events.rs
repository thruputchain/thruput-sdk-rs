//! Pausable ERC20 events, mirroring OpenZeppelin's `Pausable`:
//!
//! - `event Paused(address account)` — emitted when the pause is triggered
//!   by `account`.
//! - `event Unpaused(address account)` — emitted when the pause is lifted
//!   by `account`.
//!
//! `account` is carried in the payload (not as a topic), matching the
//! Solidity events, which do not index it.

use core::fmt;
use thruput_sdk::{
    ContractAddress, EmittedEvent, Event, hex_encode, log_event,
};

/// Log that `account` paused the token.
/// An event emitted when the token is paused.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct PausedEvent {
    /// The caller that triggered the pause.
    pub account: ContractAddress,
}

impl PausedEvent {
    pub fn log(&self) {
        log_event(self);
    }

    pub fn new(account: ContractAddress) -> Self {
        Self { account }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for PausedEvent {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != PausedEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let account: ContractAddress =
            thruput_sdk::internal::decode_xdr(payload)
                .map_err(|_| "Couldn't parse event payload")?;

        Ok(PausedEvent::new(account))
    }
}

impl fmt::Display for PausedEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PAUSED({})", hex_encode(&self.account))
    }
}

// ----------------------------------------------------------------------

/// Log that `account` unpaused the token.
/// An event emitted when the token is unpaused.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct UnpausedEvent {
    /// The caller that lifted the pause.
    pub account: ContractAddress,
}

impl UnpausedEvent {
    pub fn log(&self) {
        log_event(self);
    }

    pub fn new(account: ContractAddress) -> Self {
        Self { account }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for UnpausedEvent {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != UnpausedEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let account: ContractAddress =
            thruput_sdk::internal::decode_xdr(payload)
                .map_err(|_| "Couldn't parse event payload")?;

        Ok(UnpausedEvent::new(account))
    }
}

impl fmt::Display for UnpausedEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UNPAUSED({})", hex_encode(&self.account))
    }
}
