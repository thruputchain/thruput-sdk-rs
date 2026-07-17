//! The SHA-256 primitive, as a trait with one implementation per backend.
//!
//! Both backends compile together when both features are on: they are distinct
//! types, not competing definitions of one function, so nothing forces a choice
//! at build time and feature unification is a non-event. The choice is made at
//! each call site instead — see the [`sha2`](crate::sha2) and
//! [`syscall`](crate::syscall) modules, which name it in the path so no caller
//! can pick a backend by accident.

/// SHA-256 of a byte slice.
///
/// Implemented by the backend marker types, which are feature-gated:
/// [`Sha2`] (`sha2`) hashes in-process; [`Syscall`] (`syscall`) delegates to
/// the host. Callers pick one; there is deliberately no default.
pub trait Sha256Backend {
    /// `sha256(bytes)`.
    fn sha256(bytes: &[u8]) -> [u8; 32];
}

/// Hashes in-process with the `sha2` crate. Host-side only: linking this into
/// a guest is a hard error (see the guard in `lib.rs`).
#[cfg(feature = "sha2")]
#[cfg_attr(docsrs, doc(cfg(feature = "sha2")))]
pub struct Sha2;

#[cfg(feature = "sha2")]
impl Sha256Backend for Sha2 {
    fn sha256(bytes: &[u8]) -> [u8; 32] {
        use ::sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.finalize().into()
    }
}

/// Hashes through the SHA256 syscall — the guest path, and the only correct one
/// inside a contract frame. Works natively too when a mock dispatch is present
/// (`syscall-mock`), which is how contracts run under `native-exec`.
#[cfg(feature = "syscall")]
#[cfg_attr(docsrs, doc(cfg(feature = "syscall")))]
pub struct Syscall;

#[cfg(feature = "syscall")]
impl Sha256Backend for Syscall {
    fn sha256(bytes: &[u8]) -> [u8; 32] {
        use core::mem::MaybeUninit;
        use groundhog_xdr::types::SHA256Hash;
        let mut out = MaybeUninit::<SHA256Hash>::uninit();
        groundhog_xdr::syscalls::uninit_SHA256(bytes, &mut out)
            .unwrap_or_else(|e| panic!("SHA256 syscall failed: {e:?}"));
        // SAFETY: the syscall returned Ok, so `out` is initialized.
        unsafe { out.assume_init() }.into_inner()
    }
}

#[cfg(all(test, feature = "sha2"))]
mod tests {
    use super::{Sha2, Sha256Backend};
    use hex_literal::hex;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            Sha2::sha256(b""),
            hex!(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            )
        );
        assert_eq!(
            Sha2::sha256(b"abc"),
            hex!(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            )
        );
    }
}
