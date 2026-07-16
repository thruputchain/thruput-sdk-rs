//! `groundhog-domain-separator` must be callable from inside contract code, not
//! just from the host. The existing coverage in `groundhog-native-exec`
//! (`tests/pubkey_auth_address.rs`) derives addresses from the host side; this
//! runs the derivation inside a real contract frame under the mock VM, so it
//! covers the no_std path a deployed contract actually takes.
//!
//! The expected address is the shared cross-implementation vector emitted by
//! `groundhog-test-vectors domain-separator` (pubkey = [0x01; 32], nonce = 1) —
//! the same value the C++ validator derives, so a change to either the domain
//! separator prefix or the CXS encoding trips this.

use groundhog_domain_separator::pubkey_account_address;
use groundhog_xdr::auth_claim::PubkeyAccountPreimage;
use groundhog_xdr::types::{Ed25519PublicKey, PublicKey};
use hex_literal::hex;
use sdk::*;

const EXPECTED: [u8; 32] =
    hex!("d27f8f2df8f61a6928caf90c0ad57a29f75d23ecae45998557b090450a1f1259");

contract_test!(pubkey_account_address_in_contract {
    let preimage = PubkeyAccountPreimage {
        pubkey: PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from([0x01u8; 32]),
        },
        nonce: 1,
    };

    assert_eq!(*pubkey_account_address(&preimage), EXPECTED);
});
