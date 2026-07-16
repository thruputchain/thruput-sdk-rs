use groundhog_domain_separator::pubkey_account_address;
use hex_literal::hex;
use sdk::*;

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

    // `groundhog-domain-separator` is built here with `syscall`, so this
    // derivation hashes through the SHA256 syscall rather than linking `sha2`
    // into the guest. The expectation is the cross-implementation vector from
    // `groundhog-test-vectors domain-separator` (pubkey = [0x01; 32], nonce =
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
}
