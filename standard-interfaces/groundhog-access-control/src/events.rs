//! AccessControl events, mirroring OpenZeppelin's `IAccessControl`:
//!
//! - `event RoleGranted(bytes32 indexed role, address indexed account,
//!   address sender)`
//! - `event RoleRevoked(bytes32 indexed role, address indexed account,
//!   address sender)`
//!
//! `sender` rides in the payload rather than as a topic: Groundhog events
//! carry at most two topics beside the signature, and the OpenZeppelin
//! third index is the least useful filter of the three.

use crate::Role;
use core::fmt;
use sdk::{ContractAddress, EmittedEvent, Event, hex_encode, log_event};

/// Log that `sender` granted `role` to `account`.
pub fn log_role_granted_event(
    role: &Role,
    account: &ContractAddress,
    sender: &ContractAddress,
) {
    log_event(&RoleGrantedEvent::new(role, account, *sender));
}

/// An event emitted when `account` is granted `role`.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct RoleGrantedEvent<'a> {
    #[topic]
    pub role: &'a Role,
    #[topic]
    pub account: &'a ContractAddress,
    /// The caller that granted the role.
    pub sender: ContractAddress,
}

impl<'a> RoleGrantedEvent<'a> {
    pub fn new(
        role: &'a Role,
        account: &'a ContractAddress,
        sender: ContractAddress,
    ) -> Self {
        Self {
            role,
            account,
            sender,
        }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for RoleGrantedEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != RoleGrantedEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let role: &Role = event.topic(1).ok_or("Missing topic")?.into();
        let account: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let sender: ContractAddress = sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(RoleGrantedEvent::new(role, account, sender))
    }
}

impl<'a> fmt::Display for RoleGrantedEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ROLE_GRANTED(role:{}, {}, by:{})",
            hex_encode(self.role),
            hex_encode(self.account),
            hex_encode(&self.sender),
        )
    }
}

// ----------------------------------------------------------------------

/// Log that `sender` revoked `role` from `account`.
pub fn log_role_revoked_event(
    role: &Role,
    account: &ContractAddress,
    sender: &ContractAddress,
) {
    log_event(&RoleRevokedEvent::new(role, account, *sender));
}

/// An event emitted when `account` loses `role`, whether revoked by an
/// admin or renounced (then `sender == account`).
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct RoleRevokedEvent<'a> {
    #[topic]
    pub role: &'a Role,
    #[topic]
    pub account: &'a ContractAddress,
    /// The caller that revoked the role.
    pub sender: ContractAddress,
}

impl<'a> RoleRevokedEvent<'a> {
    pub fn new(
        role: &'a Role,
        account: &'a ContractAddress,
        sender: ContractAddress,
    ) -> Self {
        Self {
            role,
            account,
            sender,
        }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for RoleRevokedEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != RoleRevokedEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let role: &Role = event.topic(1).ok_or("Missing topic")?.into();
        let account: &ContractAddress =
            event.topic(2).ok_or("Missing topic")?.into();

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let sender: ContractAddress = sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(RoleRevokedEvent::new(role, account, sender))
    }
}

impl<'a> fmt::Display for RoleRevokedEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ROLE_REVOKED(role:{}, {}, by:{})",
            hex_encode(self.role),
            hex_encode(self.account),
            hex_encode(&self.sender),
        )
    }
}
