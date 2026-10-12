//! ERC721 errors, following the ERC-6093 custom-error vocabulary that
//! OpenZeppelin's `IERC721Errors` declares. Thruput exceptions carry no
//! arguments, so the offending address or token id is not repeated in the
//! error; the caller passed it and can name it.

#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum ERC721Error {
    /// The token id has no owner: never minted, or burned.
    #[cfg_attr(feature = "dev", error("Token does not exist"))]
    NonexistentToken = 1,

    /// The `from` of a transfer is not the token's owner.
    #[cfg_attr(feature = "dev", error("From address is not the token owner"))]
    IncorrectOwner = 2,

    /// A safe transfer's destination cannot take custody: no contract at the
    /// address, or one without an `on_erc721_received` entrypoint.
    #[cfg_attr(feature = "dev", error("Receiver cannot accept ERC721 tokens"))]
    InvalidReceiver = 3,

    /// The caller is neither the owner, the token's approved address, nor an
    /// operator approved for all of the owner's tokens.
    #[cfg_attr(
        feature = "dev",
        error("Caller is not approved for this token")
    )]
    InsufficientApproval = 4,

    /// `approve` called by someone other than the owner or an approved
    /// operator.
    #[cfg_attr(feature = "dev", error("Caller may not approve this token"))]
    InvalidApprover = 5,

    /// `token_uri` for a token whose metadata is on chain, so there is no
    /// URI; `token_metadata` has the fields.
    #[cfg_attr(
        feature = "dev",
        error("Token metadata is on chain, not at a URI")
    )]
    NoTokenUri = 6,
}
