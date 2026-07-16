use groundhog_domain_separator::{PUBKEY_ACCOUNT, pubkey_account_address};
use groundhog_xdr::auth_claim::PubkeyAccountPreimage;
use groundhog_xdr::types::{Ed25519PublicKey, PublicKey};
use serde_json::{Value, json};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct Case {
    pubkey: [u8; 32],
    nonce: u64,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            pubkey: [0u8; 32],
            nonce: 0,
        },
        Case {
            pubkey: [0xffu8; 32],
            nonce: u64::MAX,
        },
        Case {
            pubkey: [0x01u8; 32],
            nonce: 1,
        },
        Case {
            pubkey: {
                let mut k = [0u8; 32];
                k[0] = 0xde;
                k[1] = 0xad;
                k[31] = 0xef;
                k
            },
            nonce: 1_000_000,
        },
    ]
}

pub fn generate() -> Value {
    let vectors: Vec<Value> = cases()
        .into_iter()
        .map(|case| {
            let preimage = PubkeyAccountPreimage {
                pubkey: PublicKey::Ed25519 {
                    ed25519: Ed25519PublicKey::from(case.pubkey),
                },
                nonce: case.nonce,
            };
            let preimage_cxs = xdrs::cxs::encode(&preimage).unwrap();
            let address = pubkey_account_address(&preimage);
            json!({
                "domain_separator": "PubkeyAccount",
                "domain_separator_cxs_hex": hex(PUBKEY_ACCOUNT.as_bytes()),
                "pubkey_hex": hex(&case.pubkey),
                "nonce": case.nonce.to_string(),
                "preimage_cxs_hex": hex(&preimage_cxs),
                "expected_address_hex": hex(&address.into_inner()),
            })
        })
        .collect();

    json!({
        "version": 1,
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
        for v in vectors {
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
