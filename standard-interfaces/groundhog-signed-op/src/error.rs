/// Why a signed operation was refused.
///
/// Every variant is a check a verifier must actually perform; a wallet that
/// cannot throw one of these is not making that check. See
/// [`check_signed_op`](crate::check_signed_op), which throws all of them.
#[sdk::contract_exception]
#[non_exhaustive]
#[cfg_attr(feature = "dev", derive(thiserror::Error))]
pub enum SignedOpError {
    /// The signature does not verify against the operation's digest under the
    /// signer's key.
    #[cfg_attr(feature = "dev", error("Invalid signature"))]
    InvalidSignature = 1,
    /// `valid_until` is in the past. A signature that never expires is one the
    /// signer cannot retract.
    #[cfg_attr(feature = "dev", error("Operation has expired"))]
    Expired = 2,
    /// The nonce is not the one this signer owes, so the operation is either a
    /// replay or out of order.
    #[cfg_attr(feature = "dev", error("Unexpected nonce"))]
    UnexpectedNonce = 3,
    /// The operation's `type_id` is not one this contract handles. Throwing
    /// rather than ignoring is deliberate: a contract that silently accepts an
    /// unknown type has no type binding at all.
    #[cfg_attr(feature = "dev", error("Unknown operation type"))]
    UnknownOperationType = 4,
}
