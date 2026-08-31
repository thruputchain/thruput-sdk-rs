//! Vectors for the signed-operation preimage.
//!
//! Emits every intermediate stage, not just the final digest, so a client that
//! drifts fails at the stage that drifted: the separator prefix, the payload
//! type's definition, the struct encoding, the signed message, or the digest.
//!
//! The `type_def_cxs_hex` field is the one with no counterpart in the other
//! vector sets. It is the CXS encoding of the payload type's
//! `ContractTypeSpec::TYPE_DEF`, which the Rust side gets from
//! `#[contract_type]` and a client has to reproduce from the contract spec it
//! already ships.

use groundhog_contract_spec::ContractTypeSpec;
use groundhog_contract_spec::view::{
    ContractFieldView, ContractStructDefView, ContractTypeDefView,
    ContractTypeView,
};
use groundhog_domain_separator::{SIGNED_OP, Sha2, SignedOp};
use groundhog_xdr::types::{
    BlockHeight, ContractAddress, NetworkHash, SHA256Hash,
};
use serde_json::{Value, json};
use xdrs::WriteXDR;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// Stand-ins for `#[contract_type]` payloads; the macro emits exactly these
// impls. Written out by hand so this crate needs no dependency on the SDK.

/// A two-field payload with an address, exercising a non-primitive field type.
#[derive(WriteXDR)]
struct Transfer {
    to: ContractAddress,
    amount: u64,
}

impl ContractTypeSpec for Transfer {
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Struct(ContractStructDefView {
            name: "Transfer",
            fields: &[
                ContractFieldView {
                    name: "to",
                    ty: ContractTypeView::Address,
                },
                ContractFieldView {
                    name: "amount",
                    ty: ContractTypeView::U64,
                },
            ],
        });
}

/// Encodes byte-identically to [`Revoke`]; only the type definition separates
/// them. This pair is the reason the binding exists.
#[derive(WriteXDR)]
struct Grant {
    amount: u64,
}

impl ContractTypeSpec for Grant {
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Struct(ContractStructDefView {
            name: "Grant",
            fields: &[ContractFieldView {
                name: "amount",
                ty: ContractTypeView::U64,
            }],
        });
}

#[derive(WriteXDR)]
struct Revoke {
    id: u64,
}

impl ContractTypeSpec for Revoke {
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Struct(ContractStructDefView {
            name: "Revoke",
            fields: &[ContractFieldView {
                name: "id",
                ty: ContractTypeView::U64,
            }],
        });
}

struct Header {
    network: [u8; 32],
    verifying_contract: [u8; 32],
    nonce: u64,
    valid_until: i64,
}

fn vector<T: WriteXDR + ContractTypeSpec>(
    label: &str,
    header: &Header,
    body: &T,
) -> Value {
    let op = SignedOp {
        network: NetworkHash::new(SHA256Hash::new(header.network)),
        verifying_contract: ContractAddress::from(header.verifying_contract),
        nonce: header.nonce,
        valid_until: BlockHeight::new(header.valid_until),
        body,
    };

    json!({
        "label": label,
        "domain_separator": "SignedOp::1",
        "domain_separator_cxs_hex": hex(SIGNED_OP.as_bytes()),
        "type_name": T::TYPE_DEF.symbol(),
        "type_def_cxs_hex": hex(&xdrs::cxs::encode(&T::TYPE_DEF).unwrap()),
        "network_hex": hex(&header.network),
        "verifying_contract_hex": hex(&header.verifying_contract),
        "nonce": header.nonce.to_string(),
        "valid_until": header.valid_until.to_string(),
        "body_cxs_hex": hex(&xdrs::cxs::encode(body).unwrap()),
        "preimage_cxs_hex": hex(&xdrs::cxs::encode(&op).unwrap()),
        "signed_message_hex": hex(&op.preimage()),
        "expected_digest_hex": hex(op.hash::<Sha2>().as_slice()),
    })
}

pub fn generate() -> Value {
    let zeroed = Header {
        network: [0u8; 32],
        verifying_contract: [0u8; 32],
        nonce: 0,
        valid_until: 0,
    };
    let typical = Header {
        network: [0x5a; 32],
        verifying_contract: [0x03; 32],
        nonce: 1,
        valid_until: 1_000,
    };
    // Widest varints: where a client assuming fixed-width integers breaks.
    let extremes = Header {
        network: [0xff; 32],
        verifying_contract: [0xee; 32],
        nonce: u64::MAX,
        valid_until: i64::MAX,
    };

    let vectors = vec![
        // Zeroed everything: catches a client that omits a field entirely,
        // since an omitted zero field still shifts the encoding.
        vector("grant_zeroed", &zeroed, &Grant { amount: 0 }),
        vector("grant_typical", &typical, &Grant { amount: 500 }),
        // Same header, same body bytes, different payload type. If a client
        // produces the same digest for these two it has no type binding.
        vector("revoke_typical", &typical, &Revoke { id: 500 }),
        vector(
            "transfer_typical",
            &typical,
            &Transfer {
                to: ContractAddress::from([0x07; 32]),
                amount: 42,
            },
        ),
        vector("grant_extremes", &extremes, &Grant { amount: u64::MAX }),
    ];

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

    fn find<'a>(vectors: &'a [Value], label: &str) -> &'a Value {
        vectors
            .iter()
            .find(|v| v["label"] == label)
            .expect("vector present")
    }

    #[test]
    fn generates_well_formed_vectors() {
        let out = generate();
        let vectors = out["vectors"].as_array().expect("vectors array");
        assert!(!vectors.is_empty());
        for v in vectors {
            assert_eq!(v["expected_digest_hex"].as_str().unwrap().len(), 64);
            let sep = v["domain_separator_cxs_hex"].as_str().unwrap();
            // 0x00 + 0x0b + "SignedOp::1" => 13 bytes => 26 hex chars.
            assert_eq!(sep.len(), 26);
            // The signed message is exactly the separator then the struct.
            let signed = v["signed_message_hex"].as_str().unwrap();
            let preimage = v["preimage_cxs_hex"].as_str().unwrap();
            assert_eq!(signed, format!("{sep}{preimage}"));
            // The type definition has to actually appear in the message.
            assert!(
                preimage.contains(v["type_def_cxs_hex"].as_str().unwrap()),
                "type_def missing from the preimage"
            );
        }
    }

    /// The vector set is only meaningful if it contains a pair whose bodies
    /// encode identically under different payload types.
    #[test]
    fn identically_encoded_bodies_get_different_digests() {
        let out = generate();
        let vectors = out["vectors"].as_array().unwrap();
        let grant = find(vectors, "grant_typical");
        let revoke = find(vectors, "revoke_typical");

        assert_eq!(grant["body_cxs_hex"], revoke["body_cxs_hex"]);
        assert_eq!(grant["nonce"], revoke["nonce"]);
        assert_ne!(grant["type_def_cxs_hex"], revoke["type_def_cxs_hex"]);
        assert_ne!(
            grant["expected_digest_hex"],
            revoke["expected_digest_hex"]
        );
    }
}
