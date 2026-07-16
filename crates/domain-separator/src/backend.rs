//! The single feature-gated primitive: SHA-256 of a byte slice.
//!
//! Exactly one of the `sha2` / `syscall` backends is compiled (enforced by the
//! `compile_error!` guards in `lib.rs`).

#[cfg(feature = "sha2")]
#[allow(dead_code)]
pub(crate) fn sha256_raw(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(feature = "syscall")]
#[allow(dead_code)]
pub(crate) fn sha256_raw(bytes: &[u8]) -> [u8; 32] {
    use core::mem::MaybeUninit;
    use groundhog_xdr::types::SHA256Hash;
    let mut out = MaybeUninit::<SHA256Hash>::uninit();
    groundhog_xdr::syscalls::uninit_SHA256(bytes, &mut out)
        .unwrap_or_else(|e| panic!("SHA256 syscall failed: {e:?}"));
    // SAFETY: the syscall returned Ok, so `out` is initialized.
    unsafe { out.assume_init() }.into_inner()
}

#[cfg(all(test, feature = "sha2"))]
mod tests {
    use super::sha256_raw;
    use hex_literal::hex;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_raw(b""),
            hex!(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            )
        );
        assert_eq!(
            sha256_raw(b"abc"),
            hex!(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            )
        );
    }
}
