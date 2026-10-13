use thruput_domain_separator::sha2;
use thruput_sdk::types::{Ed25519PublicKey, PubkeyAccountPreimage, PublicKey};

#[test]
fn hasher_test() {
    thruput_sdk::client_test(
        |env| hasher::register_mock_contract(env),
        |c: &hasher::HasherClient| c.run(),
    );
}

/// Exercises `thruput-domain-separator`'s SHA256 syscall backend from inside
/// a contract, against the cross-implementation vector the C++ validator
/// derives.
#[test]
fn pubkey_account_address_test() {
    thruput_sdk::client_test(
        |env| hasher::register_mock_contract(env),
        |c: &hasher::HasherClient| c.check_pubkey_account_address(),
    );
}

/// Mirrors the preimage `derive_pubkey_account_address` builds contract-side.
fn preimage(pubkey_byte: u32, nonce: u64) -> PubkeyAccountPreimage {
    PubkeyAccountPreimage {
        pubkey: PublicKey::Ed25519 {
            ed25519: Ed25519PublicKey::from([pubkey_byte as u8; 32]),
        },
        nonce,
    }
}

/// Differential test: the `syscall` backend, running through the mock SHA256
/// syscall in a real contract frame, must agree with the `sha2` backend running
/// in-process. The pinned vector above catches both backends drifting together
/// from the C++ validator; this catches them drifting from *each other*, across
/// inputs no hardcoded vector covers.
///
/// Both backends coexist in this one test binary only because they are additive
/// types rather than competing definitions: the contract links `syscall`, this
/// test's dev-dependency adds `sha2`, and dev-dependencies are never built for
/// rv32 — so the guest still cannot reach `sha2`.
#[test]
fn syscall_matches_sha2() {
    let cases = [
        (0x00u32, 0u64),
        (0x01, 1),
        (0xff, u64::MAX),
        (0xab, 0),
        (0x00, u64::MAX),
    ];

    for (pubkey_byte, nonce) in cases {
        let via_sha2 =
            sha2::pubkey_account_address(&preimage(pubkey_byte, nonce));

        thruput_sdk::client_test(
            |env| hasher::register_mock_contract(env),
            move |c: &hasher::HasherClient| {
                assert_eq!(
                    c.derive_pubkey_account_address(pubkey_byte, nonce),
                    via_sha2,
                    "syscall and sha2 disagree for pubkey [{pubkey_byte:#04x}; 32], nonce {nonce}"
                );
            },
        );
    }
}
