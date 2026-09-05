#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]
//! ERC721 non-fungible tokens for Groundhog: the token interface, its
//! optional metadata, and the receiver hook behind safe transfers.
//!
//! The shapes follow [EIP-721] and OpenZeppelin's `IERC721` so that a
//! contract ported from Solidity keeps its vocabulary: `owner_of`,
//! `transfer_from`, `approve` and `set_approval_for_all` mean what they
//! mean there and a token id is a [`U256`]. Minting and burning are their
//! own events rather than transfers from or to the zero address, and the
//! zero address is an ordinary destination; its one role is standing for
//! "no approval" in [`ERC721::get_approved`] and [`ERC721::approve`], as
//! `address(0)` does in Solidity.
//!
//! # Safe transfers
//!
//! Groundhog has no externally owned accounts: every address is a contract
//! or nothing. A token sent to an address with no contract, or to one with
//! no way to move it on, is stuck. [`ERC721::safe_transfer_from`] guards
//! against that the way Ethereum does, by calling
//! [`ERC721Receiver::on_erc721_received`] on the destination after the
//! transfer and throwing [`ERC721Error::InvalidReceiver`] when the call
//! cannot be made. Where Solidity's hook returns a magic selector to say
//! "yes", here the callee simply returns; throwing rejects the transfer and
//! the exception propagates to the caller unchanged, so a receiver can
//! state its reason.
//!
//! The runtime rejects reentrancy, so a receiver that calls back into the
//! token inside the hook (to forward the token, say) faults instead. Record
//! what arrived and act on it in a later call.
//!
//! [EIP-721]: https://eips.ethereum.org/EIPS/eip-721

mod error;
pub use error::*;

mod events;
pub use events::*;

use sdk::{ContractAddress, U256, contract};

/// Identifies one token within a contract.
pub type TokenId = U256;

/// Upper bound on [`TransferData`], in bytes.
pub const TRANSFER_DATA_MAX: usize = 256;

/// Opaque bytes a safe transfer hands to the receiver hook.
///
/// Bounded so that a receiver decodes it with no allocator; Solidity's
/// `bytes data` is bounded only by gas. A caller with more to say makes a
/// separate call to the receiver.
pub type TransferData = heapless::Vec<u8, TRANSFER_DATA_MAX>;

/// A collection's name.
pub type Name = heapless::String<100>;
/// A collection's ticker symbol.
pub type Symbol = heapless::String<10>;
/// A token's metadata URI.
pub type TokenUri = heapless::String<256>;

/// The address [`ERC721::get_approved`] returns for a token with no
/// approval, and that [`ERC721::approve`] takes to clear one. It is an
/// ordinary address everywhere else.
pub const CONTRACT_ADDR_ZERO: &ContractAddress =
    &ContractAddress::new([0; 32]);

/// The standard ERC721 interface.
///
/// # Error contract
///
/// Implementations must throw exactly these [`ERC721Error`]s:
///
/// - [`ERC721Error::NonexistentToken`] — any query or transfer naming a
///   token id that has no owner.
/// - [`ERC721Error::IncorrectOwner`] — a transfer whose `from` is not the
///   token's owner.
/// - [`ERC721Error::InsufficientApproval`] — a transfer by a caller who is
///   neither the owner, the approved address, nor an approved operator.
/// - [`ERC721Error::InvalidApprover`] — `approve` by a caller who is
///   neither the owner nor an approved operator.
/// - [`ERC721Error::InvalidReceiver`] — a safe transfer to an address
///   without an `on_erc721_received` entrypoint.
///
/// A receiver hook that throws propagates its own exception through
/// `safe_transfer_from`, outside this list.
#[contract(
    client = ERC721Client,
    throws = ERC721Error,
    emits(
        TransferEvent,
        ApprovalEvent,
        ApprovalForAllEvent,
        MintEvent,
        BurnEvent
    )
)]
pub trait ERC721 {
    /// Number of tokens `owner` holds.
    fn balance_of(&self, owner: &ContractAddress) -> u64;

    /// Owner of `token_id`. Throws [`ERC721Error::NonexistentToken`].
    fn owner_of(&self, token_id: TokenId) -> ContractAddress;

    /// Move `token_id` from `from` to `to`, then call
    /// [`ERC721Receiver::on_erc721_received`] on `to` with `data`.
    ///
    /// Same authorization and errors as [`transfer_from`](Self::transfer_from),
    /// plus [`ERC721Error::InvalidReceiver`] when `to` has no contract or
    /// no receiver hook. A hook that throws rejects the transfer with its
    /// own exception.
    fn safe_transfer_from(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        token_id: TokenId,
        data: TransferData,
    );

    /// Move `token_id` from `from` to `to`, clearing its approval, and emit
    /// [`TransferEvent`].
    ///
    /// The caller must be the owner, the token's approved address, or an
    /// operator approved by the owner. Throws
    /// [`ERC721Error::NonexistentToken`], [`ERC721Error::IncorrectOwner`]
    /// when `from` does not own the token,
    /// [`ERC721Error::InsufficientApproval`] otherwise.
    fn transfer_from(
        &mut self,
        from: &ContractAddress,
        to: &ContractAddress,
        token_id: TokenId,
    );

    /// Let `to` transfer `token_id`, replacing any previous approval, and
    /// emit [`ApprovalEvent`]. [`CONTRACT_ADDR_ZERO`] clears it.
    ///
    /// The caller must be the owner or an approved operator. Throws
    /// [`ERC721Error::NonexistentToken`] and [`ERC721Error::InvalidApprover`].
    fn approve(&mut self, to: &ContractAddress, token_id: TokenId);

    /// Grant or revoke `operator` over every token the caller owns, now and
    /// later, and emit [`ApprovalForAllEvent`].
    fn set_approval_for_all(
        &mut self,
        operator: &ContractAddress,
        approved: bool,
    );

    /// The address approved for `token_id`, or [`CONTRACT_ADDR_ZERO`].
    /// Throws [`ERC721Error::NonexistentToken`].
    fn get_approved(&self, token_id: TokenId) -> ContractAddress;

    /// True when `operator` is approved for all of `owner`'s tokens.
    fn is_approved_for_all(
        &self,
        owner: &ContractAddress,
        operator: &ContractAddress,
    ) -> bool;
}

/// The optional metadata extension: a name and symbol for the collection
/// and a URI per token.
#[contract(client = ERC721MetadataClient, throws = ERC721Error)]
pub trait ERC721Metadata {
    /// The collection's name.
    fn name(&self) -> Name;

    /// The collection's symbol.
    fn symbol(&self) -> Symbol;

    /// The metadata URI for `token_id`. Throws
    /// [`ERC721Error::NonexistentToken`].
    fn token_uri(&self, token_id: TokenId) -> TokenUri;
}

/// The hook a contract implements to take custody of tokens through
/// [`ERC721::safe_transfer_from`].
///
/// Returning accepts the token; throwing rejects the transfer, and the
/// exception reaches the transfer's caller unchanged. The token has already
/// moved when the hook runs, and the runtime rejects reentrancy, so the
/// hook cannot call back into the token contract; record the arrival and
/// act on it in a later call.
#[contract(client = ERC721ReceiverClient)]
pub trait ERC721Receiver {
    /// Called by the token contract after `operator` moved `token_id` from
    /// `from` to this contract. `data` is whatever the transfer's caller
    /// attached.
    fn on_erc721_received(
        &mut self,
        operator: &ContractAddress,
        from: &ContractAddress,
        token_id: TokenId,
        data: TransferData,
    );
}
