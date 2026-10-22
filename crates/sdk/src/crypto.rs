//! System functions: hash and signature

pub use crate::types::SHA256Hash;

use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SyscallNo};
use crate::types::{
    CryptographicScheme, Ed25519PublicKey, Ed25519Signature, MlDsa65PublicKey,
    MlDsa65Signature, PublicKey, Signature,
};

pub fn sha256(input: &[u8]) -> SHA256Hash {
    let mut out_hash = core::mem::MaybeUninit::<SHA256Hash>::uninit();

    sys::uninit_SHA256(input, &mut out_hash)
        .or_syscall_throw(SyscallNo::kSHA256);

    unsafe { out_hash.assume_init() }
}

pub fn verify_signature_ed25519(
    pk: &Ed25519PublicKey,
    signature: &Ed25519Signature,
    message: &[u8],
) -> bool {
    sys::uninit_VerifySignature(
        CryptographicScheme::Ed25519.0,
        signature.as_slice(),
        pk.as_slice(),
        message,
    )
    .is_ok()
}

/// Verify a pure ML-DSA-65 (FIPS 204) signature with an empty context over
/// `message`, the same check transaction authentication performs.
///
/// Priced separately from Ed25519: 46,592 gas plus 160 per 136-byte SHAKE256
/// block of the hashed input (a 66-byte prefix plus `message`), charged
/// whether or not the signature verifies. Needs a runtime that includes hogd
/// #4541; an older runtime refuses the scheme and this returns `false`.
pub fn verify_signature_mldsa65(
    pk: &MlDsa65PublicKey,
    signature: &MlDsa65Signature,
    message: &[u8],
) -> bool {
    sys::uninit_VerifySignature(
        CryptographicScheme::MlDsa65.0,
        signature.as_slice(),
        pk.as_slice(),
        message,
    )
    .is_ok()
}

/// Verify `signature` over `message` under `pk`, whatever its scheme.
///
/// Returns `false` without a syscall when the key and signature are of
/// different schemes, or of a scheme the SDK does not know.
pub fn verify_signature(
    pk: &PublicKey,
    signature: &Signature,
    message: &[u8],
) -> bool {
    match (pk, signature) {
        (
            PublicKey::Ed25519 { ed25519: pk },
            Signature::Ed25519 { ed25519: signature },
        ) => verify_signature_ed25519(pk, signature, message),
        (
            PublicKey::MlDsa65 { mldsa65: pk },
            Signature::MlDsa65 { mldsa65: signature },
        ) => verify_signature_mldsa65(pk, signature, message),
        _ => false,
    }
}
