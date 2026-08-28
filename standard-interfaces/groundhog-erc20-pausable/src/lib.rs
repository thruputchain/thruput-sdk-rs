#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

mod error;
pub use error::*;

mod events;
pub use events::*;

use sdk::contract;

/// The pausable extension to the standard ERC20 interface.
///
/// A paused token rejects every balance-changing ERC20 operation
/// (`transfer`, `transfer_from`, `mint`, `burn`) with
/// [`ERC20PausableError::EnforcedPause`]; allowance adjustments and reads
/// stay available. Who counts as a pauser is implementation-defined (a
/// stored administrator, an access-control role, ...), but unauthorized
/// callers must be rejected with [`ERC20PausableError::MustBePauser`].
///
/// # Error contract
///
/// Implementations must throw exactly these [`ERC20PausableError`]s:
///
/// - [`ERC20PausableError::MustBePauser`] — `pause`/`unpause` called by
///   anyone the implementation does not recognize as a pauser.
/// - [`ERC20PausableError::EnforcedPause`] — a balance-changing ERC20
///   operation (or `pause` itself) while the token is already paused.
/// - [`ERC20PausableError::ExpectedPause`] — `unpause` while the token is
///   not paused.
#[contract(
    client = ERC20PausableClient,
    throws = ERC20PausableError,
    emits(PausedEvent, UnpausedEvent)
)]
pub trait ERC20Pausable {
    /// Pause the token and emit [`PausedEvent`].
    /// Throws [`ERC20PausableError::MustBePauser`] unless the caller is a
    /// pauser, and [`ERC20PausableError::EnforcedPause`] if the token is
    /// already paused.
    fn pause(&mut self);

    /// Unpause the token and emit [`UnpausedEvent`].
    /// Throws [`ERC20PausableError::MustBePauser`] unless the caller is a
    /// pauser, and [`ERC20PausableError::ExpectedPause`] if the token is
    /// not paused.
    fn unpause(&mut self);

    /// True while the token is paused.
    fn paused(&self) -> bool;
}
