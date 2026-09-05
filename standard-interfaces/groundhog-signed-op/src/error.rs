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
    /// the key the verifying contract requires. Also what a signature from
    /// any other key, or a wrongly-typed or tampered payload, produces.
    #[cfg_attr(feature = "dev", error("Invalid signature"))]
    InvalidSignature = 1,
    /// `block_expiry_height` is in the past.
    #[cfg_attr(feature = "dev", error("Operation has expired"))]
    Expired = 2,
    /// The `(nonce, block_expiry_height)` pair has already been consumed at
    /// this contract.
    #[cfg_attr(feature = "dev", error("Operation already consumed"))]
    AlreadyConsumed = 3,
    /// The body did not decode as the expected payload type.
    #[cfg_attr(feature = "dev", error("Malformed operation body"))]
    MalformedBody = 4,
    /// A consumed entry was asked to be pruned before its
    /// `block_expiry_height` had passed.
    #[cfg_attr(feature = "dev", error("Operation has not expired yet"))]
    NotYetExpired = 5,
}
