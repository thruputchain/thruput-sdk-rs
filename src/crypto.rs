//! System functions: hash and signature

pub use crate::types::SHA256Hash;

use crate::sysresult::SysResult as Result;
use crate::types::{CryptographicScheme, Ed25519PublicKey, Ed25519Signature};
use crate::sys;

pub fn sha_256(input: &[u8]) -> Result<SHA256Hash> {
    let mut out_hash = core::mem::MaybeUninit::<SHA256Hash>::uninit();
    sys::uninit_SHA256(input, &mut out_hash)?;
    let hash = unsafe { out_hash.assume_init() };
    Ok(hash)
}

#[cfg(not(test))]
pub fn sha256<S: AsRef<[u8]>>(input: S) -> SHA256Hash {
    let mut out_hash = core::mem::MaybeUninit::<SHA256Hash>::uninit();

    sys::uninit_SHA256(input.as_ref(), &mut out_hash)
        .unwrap_or_else(|e| panic!("Hash syscall failed: {}", e));

    unsafe { out_hash.assume_init() }
}

#[cfg(test)]
pub fn sha256<S: AsRef<[u8]>>(input: S) -> SHA256Hash {
    use sha2::{Sha256, Digest};

    let output: [u8; 32] = Sha256::digest(input.as_ref()).into();

    output.into()
}

pub fn verify_signature_ed25519(
    pk: &Ed25519PublicKey,
    signature: &Ed25519Signature,
    message: &[u8],
) -> Result<()> {
    sys::uninit_VerifySignature(
        CryptographicScheme::Ed25519 as i32,
        signature.as_slice(),
        pk.as_slice(),
        message,
    )
}
