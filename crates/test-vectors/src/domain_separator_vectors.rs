use serde_json::{Value, json};
use thruput_domain_separator::sha2::{
    pubkey_account_address, public_key_hash,
};
use thruput_domain_separator::{PUBKEY_ACCOUNT, PUBLIC_KEY};
use thruput_xdr::auth_claim::PubkeyAccountPreimage;
use thruput_xdr::types::{Ed25519PublicKey, MlDsa65PublicKey, PublicKey};

use crate::hex;

struct Case {
    pubkey: PublicKey,
    nonce: u64,
}

fn ed25519(key: [u8; 32]) -> PublicKey {
    PublicKey::Ed25519 {
        ed25519: Ed25519PublicKey::from(key),
    }
}

/// Not a valid ML-DSA-65 key: address derivation only hashes the bytes, so a
/// patterned fill is enough and keeps the vectors reproducible.
fn mldsa65(f: impl Fn(usize) -> u8) -> PublicKey {
    PublicKey::MlDsa65 {
        mldsa65: MlDsa65PublicKey::from(core::array::from_fn(f)),
    }
}

fn scheme_name(pubkey: &PublicKey) -> &'static str {
    match pubkey {
        PublicKey::Ed25519 { .. } => "Ed25519",
        PublicKey::MlDsa65 { .. } => "MlDsa65",
        _ => unreachable!("no vectors for this scheme"),
    }
}

fn key_bytes(pubkey: &PublicKey) -> &[u8] {
    match pubkey {
        PublicKey::Ed25519 { ed25519 } => ed25519.as_ref(),
        PublicKey::MlDsa65 { mldsa65 } => mldsa65.as_ref(),
        _ => unreachable!("no vectors for this scheme"),
    }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            pubkey: ed25519([0u8; 32]),
            nonce: 0,
        },
        Case {
            pubkey: ed25519([0xffu8; 32]),
            nonce: u64::MAX,
        },
        Case {
            pubkey: ed25519([0x01u8; 32]),
            nonce: 1,
        },
        Case {
            pubkey: ed25519({
                let mut k = [0u8; 32];
                k[0] = 0xde;
                k[1] = 0xad;
                k[31] = 0xef;
                k
            }),
            nonce: 1_000_000,
        },
        Case {
            pubkey: mldsa65(|_| 0),
            nonce: 0,
        },
        Case {
            pubkey: mldsa65(|i| i as u8),
            nonce: 42,
        },
        // Same leading 32 bytes as the second Ed25519 case: the scheme is in
        // the hash, so the addresses must differ.
        Case {
            pubkey: mldsa65(|_| 0xff),
            nonce: u64::MAX,
        },
    ]
}

pub fn generate() -> Value {
    let vectors: Vec<Value> = cases()
        .into_iter()
        .map(|case| {
            let pubkey_hash = public_key_hash(&case.pubkey);
            let preimage = PubkeyAccountPreimage {
                pubkey_hash,
                nonce: case.nonce,
            };
            let preimage_cxs = xdrs::cxs::encode(&preimage).unwrap();
            let address = pubkey_account_address(&preimage);
            json!({
                "domain_separator": "PubkeyAccount",
                "domain_separator_cxs_hex": hex(PUBKEY_ACCOUNT.as_bytes()),
                "pubkey_domain_separator_cxs_hex": hex(PUBLIC_KEY.as_bytes()),
                "scheme": scheme_name(&case.pubkey),
                "pubkey_hex": hex(key_bytes(&case.pubkey)),
                "pubkey_cxs_hex": hex(&xdrs::cxs::encode(&case.pubkey).unwrap()),
                "pubkey_hash_hex": hex(pubkey_hash.as_ref()),
                "nonce": case.nonce.to_string(),
                "preimage_cxs_hex": hex(&preimage_cxs),
                "expected_address_hex": hex(&address.into_inner()),
            })
        })
        .collect();

    json!({
        "version": 2,
        "category": "domain_separator_pubkey_account",
        "vectors": vectors,
    })
}

pub fn print() {
    println!("{}", serde_json::to_string_pretty(&generate()).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_non_empty_vectors_with_32_byte_addresses() {
        let out = generate();
        let vectors = out["vectors"].as_array().expect("vectors array");
        assert!(!vectors.is_empty());
        let mut addresses = std::collections::HashSet::new();
        for v in vectors {
            assert!(addresses.insert(v["expected_address_hex"].clone()));
            assert_eq!(v["pubkey_hash_hex"].as_str().unwrap().len(), 64);
            // 32-byte address => 64 hex chars.
            assert_eq!(v["expected_address_hex"].as_str().unwrap().len(), 64);
            // 15-byte prefix => 30 hex chars (0x00 + 0x0d + "PubkeyAccount").
            assert_eq!(
                v["domain_separator_cxs_hex"].as_str().unwrap().len(),
                30
            );
        }
    }
}
