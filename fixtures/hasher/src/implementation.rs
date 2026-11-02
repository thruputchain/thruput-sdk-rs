use hex_literal::hex;
use thruput_domain_separator::syscall::{
    pubkey_account_address, public_key_hash,
};
use thruput_sdk::*;

#[derive(Default)]
pub struct HasherImpl;

#[contract(client = HasherClient)]
impl HasherImpl {
    pub fn run() {
        const EXPECTED: SHA256Hash =
            SHA256Hash::new(hash_to_array!("foo-bar-baz"));

        let mut h = Hasher::new();
        h.update(b"foo");
        h.update(b"-bar");
        h.update(b"-baz");

        let output = h.finish();

        if output != EXPECTED {
            panic!("Unexpected hash output");
        }
    }

    // The `syscall::` path hashes through the SHA256 syscall rather than
    // linking `sha2` into the guest; on rv32 reaching the `sha2` backend
    // at all is a compile error. The expectation is the cross-implementation
    // vectors from
    // `thruput-test-vectors domain-separator` — the addresses the C++
    // validator derives. Both the key hash and the address go through the
    // syscall; the ML-DSA-65 case also exercises the largest key scratch.
    pub fn check_pubkey_account_address() {
        // Ed25519, pubkey = [0x01; 32], nonce = 1.
        const EXPECTED_ED25519: ContractAddress = ContractAddress::new(hex!(
            "b164829173b8d1a3313aff7f2d928f034b4e791ad550c90a25c16622dd6798f4"
        ));
        // ML-DSA-65, pubkey = [0xff; 1952], nonce = u64::MAX.
        const EXPECTED_MLDSA65: ContractAddress = ContractAddress::new(hex!(
            "822c12471653ddcca5ed253dde1189531aa32ffd480e367fb38d188d06354d7c"
        ));

        let ed25519 = PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from([0x01u8; 32]),
        };
        let preimage = PubkeyAccountPreimage {
            pubkey_hash: public_key_hash(&ed25519),
            nonce: 1,
        };
        if pubkey_account_address(&preimage) != EXPECTED_ED25519 {
            panic!("Unexpected Ed25519 pubkey account address");
        }

        let mldsa65 = PublicKey::MlDsa65 {
            mldsa65: MlDsa65PublicKey::from([0xffu8; 1952]).into(),
        };
        let preimage = PubkeyAccountPreimage {
            pubkey_hash: public_key_hash(&mldsa65),
            nonce: u64::MAX,
        };
        if pubkey_account_address(&preimage) != EXPECTED_MLDSA65 {
            panic!("Unexpected ML-DSA-65 pubkey account address");
        }
    }

    /// Derives an address through the SHA256 syscall and hands it back, so a
    /// host test can diff the guest backend against `sha2`. The contract cannot
    /// run that diff itself: reaching `sha2` from a guest is a compile error,
    /// which is the point of the split.
    ///
    /// Takes the pubkey as a repeated byte rather than a `PubkeyAccountPreimage`
    /// because the preimage does not implement the auth-handle traits calldata
    /// requires; the test rebuilds the identical preimage host-side. The byte
    /// rides in a `u32` because XDR has no 1-byte wire type.
    pub fn derive_pubkey_account_address(
        pubkey_byte: u32,
        nonce: u64,
    ) -> ContractAddress {
        let pubkey = PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from([pubkey_byte as u8; 32]),
        };
        let preimage = PubkeyAccountPreimage {
            pubkey_hash: public_key_hash(&pubkey),
            nonce,
        };
        pubkey_account_address(&preimage)
    }
}
