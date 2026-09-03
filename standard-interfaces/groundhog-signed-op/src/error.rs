/// Why a signed operation was refused.
///
/// There is no "wrong operation type" variant: the payload type's definition
/// is part of the signed bytes, so a message of another shape fails as
/// [`InvalidSignature`](Self::InvalidSignature).
#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum SignedOpError {
    /// The signature does not verify against the operation's preimage under
    /// the signer's key. Also what a wrongly-typed or tampered payload
    /// produces.
    #[cfg_attr(feature = "dev", error("Invalid signature"))]
    InvalidSignature = 1,
    /// `valid_until` is in the past.
    #[cfg_attr(feature = "dev", error("Operation has expired"))]
    Expired = 2,
    /// `nonce` is not the signer's next sequence number at this contract.
    #[cfg_attr(feature = "dev", error("Unexpected nonce"))]
    UnexpectedNonce = 3,
    /// The body did not decode as the expected payload type.
    #[cfg_attr(feature = "dev", error("Malformed operation body"))]
    MalformedBody = 4,
}
