#![cfg_attr(not(feature = "dev"), no_std)]
#![allow(clippy::result_large_err)]
//! ERC721 non-fungible tokens for Groundhog: the token interface, its
//! optional metadata, and the receiver hook behind safe transfers.
//!
//! The shapes follow [EIP-721] and OpenZeppelin's `IERC721` so that a
//! contract ported from Solidity keeps its vocabulary: `owner_of`,
//! `transfer_from`, `approve` and `set_approval_for_all` mean what they
//! mean there. A token id is a [`SHA256Hash`] rather than a `uint256`: 32
//! opaque bytes, the same shape as an address or a role, so a collection
//! may number its tokens, hash their content, or mint under any other
//! scheme. Minting and burning are their own events rather than transfers
//! from or to the zero address. Metadata may live at a URI, as `tokenURI`
//! has it, or on chain as [`TokenMetadata`] spells out. The
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

use sdk::{ContractAddress, SHA256Hash, contract};

/// Identifies one token within a contract: 32 opaque bytes, where
/// Ethereum uses a `uint256`. How a collection assigns them is its own
/// business.
pub type TokenId = SHA256Hash;

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

/// A token's display name, when its metadata is on chain.
pub type MetadataName = heapless::String<100>;
/// A token's description, when its metadata is on chain.
pub type MetadataDescription = heapless::String<1024>;
/// A token's image, when its metadata is on chain: a URI, `data:` URIs
/// included.
pub type MetadataImage = heapless::String<512>;

/// Token metadata stored on chain: the fields of the ERC721 metadata JSON
/// schema, as values rather than a document at a URI.
#[sdk::contract_type]
#[cfg_attr(feature = "dev", derive(Debug, PartialEq, Eq))]
pub struct OnchainMetadata {
    pub name: MetadataName,
    pub description: MetadataDescription,
    pub image: MetadataImage,
}

/// Where a token's metadata lives.
///
/// Ethereum keeps metadata off chain behind `tokenURI` because storage is
/// scarce there. Groundhog's is not, so a collection may store the fields
/// themselves. [`ERC721Metadata::token_metadata`] returns either form;
/// [`ERC721Metadata::token_uri`] keeps the ERC721 shape for the off-chain
/// one.
#[sdk::contract_type]
#[cfg_attr(feature = "dev", derive(Debug, PartialEq, Eq))]
// A contract type is XDR-encoded and `no_std`, so the on-chain variant
// cannot be boxed; the enum is built, encoded, and dropped within one call.
#[allow(clippy::large_enum_variant)]
pub enum TokenMetadata {
    /// A document at this URI, as ERC721's `tokenURI` has it.
    Uri(TokenUri) = 1,
    /// The fields themselves, in the contract's storage.
    Onchain(OnchainMetadata) = 2,
}

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
    fn owner_of(&self, token_id: &TokenId) -> ContractAddress;

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
        token_id: &TokenId,
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
        token_id: &TokenId,
    );

    /// Let `to` transfer `token_id`, replacing any previous approval, and
    /// emit [`ApprovalEvent`]. [`CONTRACT_ADDR_ZERO`] clears it.
    ///
    /// The caller must be the owner or an approved operator. Throws
    /// [`ERC721Error::NonexistentToken`] and [`ERC721Error::InvalidApprover`].
    fn approve(&mut self, to: &ContractAddress, token_id: &TokenId);

    /// Grant or revoke `operator` over every token the caller owns, now and
    /// later, and emit [`ApprovalForAllEvent`].
    fn set_approval_for_all(
        &mut self,
        operator: &ContractAddress,
        approved: bool,
    );

    /// The address approved for `token_id`, or [`CONTRACT_ADDR_ZERO`].
    /// Throws [`ERC721Error::NonexistentToken`].
    fn get_approved(&self, token_id: &TokenId) -> ContractAddress;

    /// True when `operator` is approved for all of `owner`'s tokens.
    fn is_approved_for_all(
        &self,
        owner: &ContractAddress,
        operator: &ContractAddress,
    ) -> bool;
}

/// The optional metadata extension: a name and symbol for the collection
/// and, per token, metadata at a URI or on chain.
///
/// # Error contract
///
/// - [`ERC721Error::NonexistentToken`] — a query naming a token id that has
///   no owner.
/// - [`ERC721Error::NoTokenUri`] — [`token_uri`](Self::token_uri) for a
///   token whose metadata is on chain.
#[contract(client = ERC721MetadataClient, throws = ERC721Error)]
pub trait ERC721Metadata {
    /// The collection's name.
    fn name(&self) -> Name;

    /// The collection's symbol.
    fn symbol(&self) -> Symbol;

    /// The metadata URI for `token_id`, for a token whose metadata is off
    /// chain. Throws [`ERC721Error::NonexistentToken`], and
    /// [`ERC721Error::NoTokenUri`] when the metadata is on chain;
    /// [`token_metadata`](Self::token_metadata) answers for either.
    fn token_uri(&self, token_id: &TokenId) -> TokenUri;

    /// The metadata for `token_id`, wherever it lives. Throws
    /// [`ERC721Error::NonexistentToken`].
    fn token_metadata(&self, token_id: &TokenId) -> TokenMetadata;
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
        token_id: &TokenId,
        data: TransferData,
    );
}
