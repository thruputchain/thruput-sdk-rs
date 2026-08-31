/// Why a signed operation was refused.
///
/// Every variant is a check a verifier must actually perform;
/// [`SignedOpNonces::verify_and_spend`](crate::SignedOpNonces::verify_and_spend)
/// throws all of them.
///
/// Note what is *not* here: there is no "wrong operation type" error. The
/// payload type's definition is part of the signed bytes, so a message of some
/// other shape fails [`InvalidSignature`](Self::InvalidSignature) rather than a
/// separate check a verifier could forget to write.
#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum SignedOpError {
    /// The signature does not verify against the operation's preimage under the
    /// signer's key. Also what a wrongly-typed or tampered payload produces.
    #[cfg_attr(feature = "dev", error("Invalid signature"))]
    InvalidSignature = 1,
    /// `valid_until` is in the past. A signature that never expires is one the
    /// signer cannot retract.
    #[cfg_attr(feature = "dev", error("Operation has expired"))]
    Expired = 2,
    /// This signer has already spent that nonce slot — the operation is a
    /// replay, or its slot was cancelled.
    #[cfg_attr(feature = "dev", error("Nonce slot already spent"))]
    NonceAlreadySpent = 3,
    /// The body did not decode as the expected payload type.
    #[cfg_attr(feature = "dev", error("Malformed operation body"))]
    MalformedBody = 4,
    /// The nonce fell out of the replay window before it was spent.
    ///
    /// An artifact of the current sliding-window filter, which forgets in order
    /// to bound its storage. A garbage-collecting replay cache would not need
    /// this variant; treat its presence as the marker for that gap.
    #[cfg_attr(feature = "dev", error("Nonce older than the replay window"))]
    NonceTooOld = 5,
}
