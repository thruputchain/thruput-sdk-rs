#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

mod error;
pub use error::*;

mod events;
pub use events::*;

use sdk::{ContractAddress, SHA256Hash, contract};

/// Identifies a role: an opaque 32-byte value, conventionally the SHA-256
/// hash of the role's name. `sdk::hash_ident!` produces one from a bare
/// identifier:
///
/// ```ignore
/// sdk::hash_ident! { pub static PAUSER_ROLE: SHA256Hash; }
/// ```
pub type Role = SHA256Hash;

/// The all-zero role that, by convention, administers other roles.
///
/// [`AccessControl::get_role_admin`] decides which role actually
/// administers a given role; implementations that keep the flat default
/// return this for every role.
pub const DEFAULT_ADMIN_ROLE: &Role = &SHA256Hash::new([0; 32]);

/// Role-based access control, following OpenZeppelin's `AccessControl`.
///
/// Each role is administered by another role: only holders of
/// [`get_role_admin`](AccessControl::get_role_admin)`(role)` may grant or
/// revoke `role`. How the role→admin mapping is wired (and whether it can
/// change after initialization) is implementation-defined; the reference
/// implementation administers every role with [`DEFAULT_ADMIN_ROLE`]. An
/// implementation that re-wires a role's admin must emit
/// [`RoleAdminChangedEvent`].
///
/// Divergence from OpenZeppelin: their
/// `renounceRole(role, callerConfirmation)` makes the caller re-pass its
/// own address, purely as a guard against reaching the function by
/// accident from raw calldata, and reverts with `BadConfirmation` on a
/// mismatch. Thruput contracts are called through typed clients where
/// that guard buys nothing, so [`renounce_role`](AccessControl::renounce_role)
/// drops the confirmation argument — it always acts on the caller — and
/// no `BadConfirmation` error exists.
///
/// # Error contract
///
/// Implementations must throw exactly these [`AccessControlError`]s:
///
/// - [`AccessControlError::MissingRole`] — `grant_role`/`revoke_role`
///   called by anyone not holding the target role's admin role.
#[contract(
    client = AccessControlClient,
    throws = AccessControlError,
    emits(RoleGrantedEvent, RoleRevokedEvent, RoleAdminChangedEvent)
)]
pub trait AccessControl {
    /// True when `account` holds `role`.
    fn has_role(&self, role: &Role, account: &ContractAddress) -> bool;

    /// The role whose holders may grant and revoke `role`.
    fn get_role_admin(&self, role: &Role) -> Role;

    /// Grant `role` to `account` and emit [`RoleGrantedEvent`].
    /// Throws [`AccessControlError::MissingRole`] unless the caller holds
    /// `role`'s admin role. Does nothing (and emits nothing) when
    /// `account` already holds `role`.
    fn grant_role(&mut self, role: &Role, account: &ContractAddress);

    /// Revoke `role` from `account` and emit [`RoleRevokedEvent`].
    /// Throws [`AccessControlError::MissingRole`] unless the caller holds
    /// `role`'s admin role. Does nothing (and emits nothing) when
    /// `account` does not hold `role`.
    fn revoke_role(&mut self, role: &Role, account: &ContractAddress);

    /// Remove `role` from the caller and emit [`RoleRevokedEvent`].
    /// Needs no admin role. Does nothing (and emits nothing) when the
    /// caller does not hold `role`.
    ///
    /// Unlike OpenZeppelin's `renounceRole`, there is no
    /// `callerConfirmation` argument (see the trait docs).
    fn renounce_role(&mut self, role: &Role);
}
