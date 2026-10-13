use thruput_domain_separator::syscall::pubkey_account_address;
use hex_literal::hex;
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
    // vector from
    // `thruput-test-vectors domain-separator` (pubkey = [0x01; 32], nonce =
    // 1) — the address the C++ validator derives.
    pub fn check_pubkey_account_address() {
        const EXPECTED_ADDRESS: ContractAddress = ContractAddress::new(hex!(
            "d27f8f2df8f61a6928caf90c0ad57a29f75d23ecae45998557b090450a1f1259"
        ));

        let preimage = PubkeyAccountPreimage {
            pubkey: PublicKey::Ed25519 {
                ed25519: Ed25519PublicKey::from([0x01u8; 32]),
            },
            nonce: 1,
        };

        if pubkey_account_address(&preimage) != EXPECTED_ADDRESS {
            panic!("Unexpected pubkey account address");
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
        let preimage = PubkeyAccountPreimage {
            pubkey: PublicKey::Ed25519 {
                ed25519: Ed25519PublicKey::from([pubkey_byte as u8; 32]),
            },
            nonce,
        };
        pubkey_account_address(&preimage)
    }
}
