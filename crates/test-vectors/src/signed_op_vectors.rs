//! Vectors for the signed-operation preimage.
//!
//! Emits every intermediate stage, not just the final digest, so a client that
//! drifts fails at the stage that drifted: the separator prefix, the derived
//! operation type ID, the preimage CXS encoding, or the digest.

use groundhog_contract_util::operation_type_id;
use groundhog_domain_separator::{SIGNED_OP, Sha2, SignedOp};
use groundhog_xdr::types::{
    BlockHeight, ContractAddress, NetworkHash, SHA256Hash,
};
use serde_json::{Value, json};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct Case {
    interface: Option<&'static str>,
    version_str: &'static str,
    operation: &'static str,
    network: [u8; 32],
    verifying_contract: [u8; 32],
    nonce: u64,
    valid_until: i64,
    body: Vec<u8>,
}

fn cases() -> Vec<Case> {
    vec![
        // Zeroed everything: catches a client that omits a field entirely,
        // since an omitted zero field still shifts the encoding.
        Case {
            interface: Some("MyWallet"),
            version_str: "0.1.0",
            operation: "transfer",
            network: [0u8; 32],
            verifying_contract: [0u8; 32],
            nonce: 0,
            valid_until: 0,
            body: Vec::new(),
        },
        // Typical.
        Case {
            interface: Some("MyWallet"),
            version_str: "0.1.0",
            operation: "transfer",
            network: [0x5a; 32],
            verifying_contract: [0x03; 32],
            nonce: 1,
            valid_until: 1_000,
            body: vec![0x2a],
        },
        // Same everything but the operation name: pins that the type ID is
        // load-bearing in the digest.
        Case {
            interface: Some("MyWallet"),
            version_str: "0.1.0",
            operation: "approve",
            network: [0x5a; 32],
            verifying_contract: [0x03; 32],
            nonce: 1,
            valid_until: 1_000,
            body: vec![0x2a],
        },
        // Widest varints: nonce and expiry both at their maxima, which is where
        // a client that assumes fixed-width integers breaks.
        Case {
            interface: Some("MyWallet"),
            version_str: "1.2.3",
            operation: "sweep",
            network: [0xff; 32],
            verifying_contract: [0xee; 32],
            nonce: u64::MAX,
            valid_until: i64::MAX,
            body: vec![0xde, 0xad, 0xbe, 0xef],
        },
        // Unnamed interface, and a longer body that crosses a varint length
        // boundary (>127 bytes) in the body's own opaque length prefix.
        Case {
            interface: None,
            version_str: "0.0.1",
            operation: "batch",
            network: [0x11; 32],
            verifying_contract: [0x22; 32],
            nonce: 42,
            valid_until: 999_999,
            body: (0u16..200).map(|i| (i % 256) as u8).collect(),
        },
    ]
}

pub fn generate() -> Value {
    let vectors: Vec<Value> = cases()
        .into_iter()
        .map(|case| {
            let type_id = operation_type_id(
                case.interface,
                case.version_str,
                case.operation,
            );
            let op = SignedOp {
                network: NetworkHash::new(SHA256Hash::new(case.network)),
                verifying_contract: ContractAddress::from(
                    case.verifying_contract,
                ),
                type_id,
                nonce: case.nonce,
                valid_until: BlockHeight::new(case.valid_until),
                body: &case.body,
            };
            let preimage_cxs = xdrs::cxs::encode(&op).unwrap();
            // What a signer actually signs: separator prefix + struct.
            let signed_message = op.preimage();
            let digest = op.hash::<Sha2>();

            json!({
                "domain_separator": "SignedOp::1",
                "domain_separator_cxs_hex": hex(SIGNED_OP.as_bytes()),
                "interface": case.interface,
                "version_str": case.version_str,
                "operation": case.operation,
                "type_id_hex": hex(type_id.as_bytes()),
                "network_hex": hex(&case.network),
                "verifying_contract_hex": hex(&case.verifying_contract),
                "nonce": case.nonce.to_string(),
                "valid_until": case.valid_until.to_string(),
                "body_hex": hex(&case.body),
                "preimage_cxs_hex": hex(&preimage_cxs),
                "signed_message_hex": hex(&signed_message),
                "expected_digest_hex": hex(digest.as_slice()),
            })
        })
        .collect();

    json!({
        "version": 1,
        "category": "signed_op",
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
    fn generates_well_formed_vectors() {
        let out = generate();
        let vectors = out["vectors"].as_array().expect("vectors array");
        assert!(!vectors.is_empty());
        for v in vectors {
            // 32-byte digest => 64 hex chars.
            assert_eq!(v["expected_digest_hex"].as_str().unwrap().len(), 64);
            // 8-byte type ID => 16 hex chars.
            assert_eq!(v["type_id_hex"].as_str().unwrap().len(), 16);
            // 0x00 + 0x0b + "SignedOp::1" => 13 bytes => 26 hex chars.
            let sep = v["domain_separator_cxs_hex"].as_str().unwrap();
            assert_eq!(sep.len(), 26);
            // The signed message is exactly the separator then the struct.
            let signed = v["signed_message_hex"].as_str().unwrap();
            let preimage = v["preimage_cxs_hex"].as_str().unwrap();
            assert_eq!(signed, format!("{sep}{preimage}"));
        }
    }

    /// The two cases differing only in operation name must not collide, or the
    /// vector set would pass for a client with no type binding at all.
    #[test]
    fn operation_name_changes_the_digest() {
        let out = generate();
        let vectors = out["vectors"].as_array().unwrap();
        let transfer = vectors
            .iter()
            .find(|v| v["operation"] == "transfer" && v["nonce"] == "1")
            .unwrap();
        let approve = vectors
            .iter()
            .find(|v| v["operation"] == "approve")
            .unwrap();
        assert_eq!(transfer["body_hex"], approve["body_hex"]);
        assert_eq!(transfer["nonce"], approve["nonce"]);
        assert_ne!(
            transfer["expected_digest_hex"],
            approve["expected_digest_hex"]
        );
    }
}
