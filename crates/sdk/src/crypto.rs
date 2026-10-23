//! System functions: hash and signature

pub use crate::types::SHA256Hash;

use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SyscallNo};
use crate::types::{
    CryptographicScheme, Ed25519PublicKey, Ed25519Signature, PublicKey,
    Signature,
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

/// Verify `signature` over `message` under `pk`, in `pk`'s scheme.
///
/// The syscall checks the key and signature lengths against that scheme, so
/// a signature of another scheme fails without being verified. ML-DSA-65 is
/// pure ML-DSA with an empty context, priced at 46,592 gas plus 160 per
/// 136-byte SHAKE256 block of a 66-byte prefix plus `message`, and needs a
/// runtime that includes hogd #4541.
pub fn verify_signature(
    pk: &PublicKey,
    signature: &Signature,
    message: &[u8],
) -> bool {
    sys::uninit_VerifySignature(
        pk.scheme().0,
        signature_bytes(signature),
        public_key_bytes(pk),
        message,
    )
    .is_ok()
}

/// The raw bytes of a key; empty for a scheme the SDK does not know, which
/// the runtime rejects.
pub(crate) fn public_key_bytes(pk: &PublicKey) -> &[u8] {
    match pk {
        PublicKey::Ed25519 { ed25519 } => ed25519.as_ref(),
        PublicKey::MlDsa65 { mldsa65 } => mldsa65.as_slice(),
        _ => &[],
    }
}

fn signature_bytes(signature: &Signature) -> &[u8] {
    match signature {
        Signature::Ed25519 { ed25519 } => ed25519.as_ref(),
        Signature::MlDsa65 { mldsa65 } => mldsa65.as_slice(),
        _ => &[],
    }
}
