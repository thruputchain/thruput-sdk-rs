#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]

mod error;
pub use error::*;

mod events;
pub use events::*;

use thruput_sdk::contract;

/// The pausable extension to the standard ERC20 interface.
///
/// A paused token rejects every balance-changing ERC20 operation
/// (`transfer`, `transfer_from`, `mint`, and any burn an extension adds) with
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
///
/// # Catching pause rejections through `ERC20Client`
///
/// [`ERC20PausableError::EnforcedPause`] deliberately stays out of the
/// core ERC20 `throws` contract, mirroring OpenZeppelin (`IERC20` does
/// not declare `EnforcedPause` either): plain ERC20 callers, and tokens
/// without this extension, are not coupled to it. Thruput exceptions
/// carry a selector that identifies their type independent of any
/// interface's `throws` list, so a typed ERC20 caller catches a pause
/// rejection as the client's `Unknown(raw)` variant and decodes it with
/// `raw.try_as::<ERC20PausableError>()` — a one-line typed decode, the
/// same mechanism a Solidity `try`/`catch` uses to match a custom error
/// selector from outside the called interface's ABI. Non-`try_` calls
/// re-throw it to the caller unchanged, so composed contracts propagate
/// pause rejections with no handling at all.
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
