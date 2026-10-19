//! AccessControl events, mirroring OpenZeppelin's `IAccessControl`:
//!
//! - `event RoleGranted(bytes32 indexed role, address indexed account,
//!   address sender)`
//! - `event RoleRevoked(bytes32 indexed role, address indexed account,
//!   address sender)`
//! - `event RoleAdminChanged(bytes32 indexed role, bytes32 indexed
//!   previousAdminRole, bytes32 indexed newAdminRole)`
//!
//! Thruput events carry at most two topics beside the signature, so the
//! last OpenZeppelin index of each event — the least useful filter of its
//! three — rides in the payload instead.

use crate::Role;
use core::fmt;
use thruput_sdk::{
    ContractAddress, EmittedEvent, Event, base58_encode, log_event,
};

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
        let sender: ContractAddress =
            thruput_sdk::internal::decode_xdr(payload)
                .map_err(|_| "Couldn't parse event payload")?;

        Ok(RoleGrantedEvent::new(role, account, sender))
    }
}

impl<'a> fmt::Display for RoleGrantedEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ROLE_GRANTED(role:{}, {}, by:{})",
            base58_encode(self.role),
            base58_encode(self.account),
            base58_encode(&self.sender),
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
        let sender: ContractAddress =
            thruput_sdk::internal::decode_xdr(payload)
                .map_err(|_| "Couldn't parse event payload")?;

        Ok(RoleRevokedEvent::new(role, account, sender))
    }
}

impl<'a> fmt::Display for RoleRevokedEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ROLE_REVOKED(role:{}, {}, by:{})",
            base58_encode(self.role),
            base58_encode(self.account),
            base58_encode(&self.sender),
        )
    }
}

// ----------------------------------------------------------------------

/// Log that `role`'s admin role changed from `previous_admin_role` to
/// `new_admin_role`.
pub fn log_role_admin_changed_event(
    role: &Role,
    previous_admin_role: &Role,
    new_admin_role: &Role,
) {
    log_event(&RoleAdminChangedEvent::new(
        role,
        previous_admin_role,
        *new_admin_role,
    ));
}

/// An event emitted when the role administering `role` changes.
///
/// Before any such event, every role is administered by whatever
/// [`AccessControl::get_role_admin`](crate::AccessControl::get_role_admin)
/// returned at initialization ([`DEFAULT_ADMIN_ROLE`](crate::DEFAULT_ADMIN_ROLE)
/// in the reference implementation).
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct RoleAdminChangedEvent<'a> {
    #[topic]
    pub role: &'a Role,
    #[topic]
    pub previous_admin_role: &'a Role,
    /// The role that administers `role` from now on.
    pub new_admin_role: Role,
}

impl<'a> RoleAdminChangedEvent<'a> {
    pub fn new(
        role: &'a Role,
        previous_admin_role: &'a Role,
        new_admin_role: Role,
    ) -> Self {
        Self {
            role,
            previous_admin_role,
            new_admin_role,
        }
    }
}

impl<'a> TryFrom<EmittedEvent<'a>> for RoleAdminChangedEvent<'a> {
    type Error = &'static str;

    fn try_from(
        event: EmittedEvent<'a>,
    ) -> core::result::Result<Self, Self::Error> {
        let sig = event.topic(0).ok_or("No event signature")?;

        if sig != RoleAdminChangedEvent::SIGNATURE {
            return Err("Unexpected event signature");
        }

        let role: &Role = event.topic(1).ok_or("Missing topic")?.into();
        let previous_admin_role: &Role =
            event.topic(2).ok_or("Missing topic")?.into();

        let payload = event
            .payload()
            .ok_or("Malformed event: topics overrun body")?;
        let new_admin_role: Role = thruput_sdk::internal::decode_xdr(payload)
            .map_err(|_| "Couldn't parse event payload")?;

        Ok(RoleAdminChangedEvent::new(
            role,
            previous_admin_role,
            new_admin_role,
        ))
    }
}

impl<'a> fmt::Display for RoleAdminChangedEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ROLE_ADMIN_CHANGED(role:{}, {} -> {})",
            base58_encode(self.role),
            base58_encode(self.previous_admin_role),
            base58_encode(&self.new_admin_role),
        )
    }
}
