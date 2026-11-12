use thruput_sdk::crypto::ristretto255_msm;
use thruput_sdk::{OrSyscallThrow, contract, sys::SyscallNo};

pub struct RistrettoMsm;

#[contract(client = RistrettoMsmClient)]
impl RistrettoMsm {
    /// Build boundary-sized inputs locally: ordinary calldata scratch is 4 KiB.
    pub fn repeated_msm(term: &[u8; 64], count: u32) -> [u8; 32] {
        assert!(count <= 257);
        let len = count as usize * 64;
        let mut terms = [0u8; 257 * 64];
        for chunk in terms[..len].as_chunks_mut::<64>().0 {
            chunk.copy_from_slice(term);
        }
        Self::msm(&terms[..len])
    }

    pub fn msm(packed_terms: &[u8]) -> [u8; 32] {
        ristretto255_msm(packed_terms)
            .or_syscall_throw(SyscallNo::kRistretto255Msm)
            .into_inner()
    }

    /// Check C = value * G + blinding * H with caller-supplied bases.
    pub fn check_opening(
        value: &[u8; 32],
        blinding: &[u8; 32],
        value_base: &[u8; 32],
        blinding_base: &[u8; 32],
        commitment: &[u8; 32],
    ) -> bool {
        let mut terms = [0u8; 128];
        terms[..32].copy_from_slice(value);
        terms[32..64].copy_from_slice(value_base);
        terms[64..96].copy_from_slice(blinding);
        terms[96..].copy_from_slice(blinding_base);
        Self::msm(&terms) == *commitment
    }
}
