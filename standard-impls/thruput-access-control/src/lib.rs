//! Embeddable implementation of the [`AccessControl`] standard interface.
//!
//! The interface crate (`thruput-access-control-if`, re-exported here) is
//! the `IAccessControl` half of the OpenZeppelin split: trait, events,
//! errors. This crate is the `AccessControl` half: the storage layout and
//! the method bodies, packaged as [`AccessControlState`] — a storage
//! component a contract embeds as a field of its own storage struct.
//! Rust has no contract inheritance, so the contract still declares the
//! `#[contract] impl AccessControl` block itself, with each method a
//! one-line delegation into the embedded state:
//!
//! ```ignore
//! #[sdk::storage]
//! pub struct MyContract {
//!     access: AccessControlState,
//!     // ...
//! }
//!
//! #[contract]
//! impl AccessControl for MyContract {
//!     fn has_role(&self, role: &Role, account: &ContractAddress) -> bool {
//!         self.access.has_role(role, account)
//!     }
//!     // ... likewise for the other trait methods
//! }
//! ```
//!
//! The `*_unchecked` methods are the equivalent of OpenZeppelin's
//! `internal` `_grantRole` / `_revokeRole` / `_setRoleAdmin`: they skip
//! the admin-role check so initializers and contract-specific flows can
//! wire roles directly, and they emit the standard events so the event
//! log stays faithful no matter which path mutated the state.

#![cfg_attr(not(feature = "dev"), no_std)]

pub use thruput_access_control_if::*;

use sdk::storage::{Map, Value};
use sdk::{ContractAddress, NoneIfMissing, ensure, get_caller};

/// Role-based access control state: who holds which role, and which role
/// administers which.
///
/// Embed as a field of a contract's storage struct. All state starts
/// empty: nobody holds any role (grant the first administrator from the
/// contract's initializer via
/// [`grant_role_unchecked`](Self::grant_role_unchecked)), and every role
/// is administered by [`DEFAULT_ADMIN_ROLE`] until
/// [`set_role_admin_unchecked`](Self::set_role_admin_unchecked) rewires
/// it.
#[sdk::storage]
#[cfg_attr(feature = "dev", derive(Debug))]
pub struct AccessControlState {
    roles: Map<Role, Map<ContractAddress, Value<bool>>>,
    role_admins: Map<Role, Value<Role>>,
}

impl AccessControlState {
    /// True when `account` holds `role`.
    pub fn has_role(&self, role: &Role, account: &ContractAddress) -> bool {
        self.roles
            .at(role)
            .at(account)
            .try_get()
            .none_if_missing()
            .unwrap_or(false)
    }

    /// The role whose holders may grant and revoke `role`:
    /// [`DEFAULT_ADMIN_ROLE`] unless
    /// [`set_role_admin_unchecked`](Self::set_role_admin_unchecked)
    /// rewired it.
    pub fn role_admin(&self, role: &Role) -> Role {
        self.role_admins
            .at(role)
            .try_get()
            .none_if_missing()
            .unwrap_or(*DEFAULT_ADMIN_ROLE)
    }

    /// Throw [`AccessControlError::MissingRole`] unless `account` holds
    /// `role`.
    pub fn require_role(&self, role: &Role, account: &ContractAddress) {
        ensure(
            self.has_role(role, account),
            AccessControlError::MissingRole,
        );
    }

    /// Grant `role` to `account`, checking that the caller holds `role`'s
    /// admin role ([`AccessControlError::MissingRole`] otherwise).
    /// Implements [`AccessControl::grant_role`].
    pub fn grant_role(&mut self, role: &Role, account: &ContractAddress) {
        self.require_role(&self.role_admin(role), &get_caller());
        self.grant_role_unchecked(role, account);
    }

    /// Revoke `role` from `account`, checking that the caller holds
    /// `role`'s admin role ([`AccessControlError::MissingRole`]
    /// otherwise). Implements [`AccessControl::revoke_role`].
    pub fn revoke_role(&mut self, role: &Role, account: &ContractAddress) {
        self.require_role(&self.role_admin(role), &get_caller());
        self.revoke_role_unchecked(role, account);
    }

    /// Remove `role` from the caller, with no admin check. Implements
    /// [`AccessControl::renounce_role`].
    pub fn renounce_role(&mut self, role: &Role) {
        self.revoke_role_unchecked(role, &get_caller());
    }

    /// Grant `role` to `account` with no admin check, emitting
    /// [`RoleGrantedEvent`] with the caller as sender. Returns whether the
    /// role was newly held; an already-held role is a no-op that emits
    /// nothing. The equivalent of OpenZeppelin's internal `_grantRole`.
    pub fn grant_role_unchecked(
        &mut self,
        role: &Role,
        account: &ContractAddress,
    ) -> bool {
        if self.has_role(role, account) {
            return false;
        }
        self.roles.at_mut(role).at_mut(account).set(&true);
        log_role_granted_event(role, account, &get_caller());
        true
    }

    /// Remove `role` from `account` with no admin check, emitting
    /// [`RoleRevokedEvent`] with the caller as sender. Returns whether the
    /// role was held; an unheld role is a no-op that emits nothing. The
    /// equivalent of OpenZeppelin's internal `_revokeRole`.
    pub fn revoke_role_unchecked(
        &mut self,
        role: &Role,
        account: &ContractAddress,
    ) -> bool {
        if !self.has_role(role, account) {
            return false;
        }
        self.roles.at_mut(role).at_mut(account).set(&false);
        log_role_revoked_event(role, account, &get_caller());
        true
    }

    /// Make `admin_role` the role administering `role`, with no check on
    /// the caller, emitting [`RoleAdminChangedEvent`]. The equivalent of
    /// OpenZeppelin's internal `_setRoleAdmin`; like it, this has no
    /// public trait counterpart — a contract that wants role-admin
    /// rewiring exposes (and guards) its own entrypoint around this.
    pub fn set_role_admin_unchecked(
        &mut self,
        role: &Role,
        admin_role: &Role,
    ) {
        let previous = self.role_admin(role);
        self.role_admins.at_mut(role).set(admin_role);
        log_role_admin_changed_event(role, &previous, admin_role);
    }
}
