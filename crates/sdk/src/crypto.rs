//! System functions: hash and signature

pub use crate::types::SHA256Hash;

use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SyscallNo};
use crate::types::{CryptographicScheme, Ed25519PublicKey, Ed25519Signature};

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
