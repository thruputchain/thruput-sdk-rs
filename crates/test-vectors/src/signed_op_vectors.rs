//! Vectors for the signed-operation preimage.
//!
//! Emits every intermediate stage, not just the signed message, so a client
//! that drifts fails at the stage that drifted. `type_def_cxs_hex` is the CXS
//! encoding of the payload type's `ContractTypeSpec::TYPE_DEF`, which the Rust
//! side gets from `#[contract_type]` and a client has to reproduce from the
//! contract spec it ships.

use serde_json::{Value, json};
use thruput_contract_spec::{
    ContractSpecIdView, ContractTypeName, ContractTypeSpec,
};
use thruput_domain_separator::{SIGNED_OP, SignedOp};
use thruput_xdr::transaction::Uniquifier;
use thruput_xdr::types::{
    BlockHeight, ContractAddress, NetworkHash, SHA256Hash,
};
use xdrs::WriteXDR;

use crate::hex;

/// Major the fixture types below carry in their spec ids.
///
/// They are fixtures, not published types -- nothing outside this crate
/// resolves against them -- so this is a fixed literal rather than the
/// crate's own major, which would churn every generated vector on each
/// breaking release for no reader's benefit.
const FIXTURE_MAJOR: &str = "0";

// Payload types are derived, not hand-written: `type_def_cxs_hex` is the
// cross-SDK contract, so a hand-written `TYPE_DEF` could pin bytes no real
// `#[contract_type]` payload produces.

/// A two-field payload with an address, exercising a non-primitive field type.
#[derive(WriteXDR, thruput_macros::ContractTypeSpec)]
#[contract_type_spec(crate = "crate::__sdk_paths")]
struct Transfer {
    to: ContractAddress,
    amount: u64,
}

impl ContractTypeName for Transfer {
    const NAME: ContractSpecIdView<'static> = ContractSpecIdView {
        crate_name: env!("CARGO_PKG_NAME"),
        major: FIXTURE_MAJOR,
        module_path: module_path!(),
        declared: "Transfer",
    };
}

/// Encodes byte-identically to [`Revoke`]; only the type definition separates
/// them.
#[derive(WriteXDR, thruput_macros::ContractTypeSpec)]
#[contract_type_spec(crate = "crate::__sdk_paths")]
struct Grant {
    amount: u64,
}

impl ContractTypeName for Grant {
    const NAME: ContractSpecIdView<'static> = ContractSpecIdView {
        crate_name: env!("CARGO_PKG_NAME"),
        major: FIXTURE_MAJOR,
        module_path: module_path!(),
        declared: "Grant",
    };
}

#[derive(WriteXDR, thruput_macros::ContractTypeSpec)]
#[contract_type_spec(crate = "crate::__sdk_paths")]
struct Revoke {
    id: u64,
}

impl ContractTypeName for Revoke {
    const NAME: ContractSpecIdView<'static> = ContractSpecIdView {
        crate_name: env!("CARGO_PKG_NAME"),
        major: FIXTURE_MAJOR,
        module_path: module_path!(),
        declared: "Revoke",
    };
}

#[derive(Clone, Copy)]
struct Header {
    network: [u8; 32],
    verifying_contract: [u8; 32],
    block_expiry_height: i64,
    nonce: u64,
}

fn vector<T: WriteXDR + ContractTypeSpec>(
    label: &str,
    header: &Header,
    body: &T,
) -> Value {
    let uniquifier = Uniquifier {
        block_expiry_height: BlockHeight::new(header.block_expiry_height),
        nonce: header.nonce,
    };
    let op = SignedOp {
        network: NetworkHash::new(SHA256Hash::new(header.network)),
        verifying_contract: ContractAddress::from(header.verifying_contract),
        uniquifier: uniquifier.clone(),
        body,
    };

    json!({
        "label": label,
        "type_name": T::TYPE_DEF.symbol().declared,
        "type_def_cxs_hex": hex(&xdrs::cxs::encode(&T::TYPE_DEF).unwrap()),
        "network_hex": hex(&header.network),
        "verifying_contract_hex": hex(&header.verifying_contract),
        // The chain's Uniquifier, encoded as it sits in the preimage
        // (expiry first, then nonce); the two fields are also given
        // separately for clients that build the pair themselves.
        "block_expiry_height": header.block_expiry_height.to_string(),
        "nonce": header.nonce.to_string(),
        "uniquifier_cxs_hex": hex(&xdrs::cxs::encode(&uniquifier).unwrap()),
        "body_cxs_hex": hex(&xdrs::cxs::encode(body).unwrap()),
        "signed_message_hex": hex(&op.preimage().unwrap()),
    })
}

pub fn generate() -> Value {
    let zeroed = Header {
        network: [0u8; 32],
        verifying_contract: [0u8; 32],
        block_expiry_height: 0,
        nonce: 0,
    };
    let typical = Header {
        network: [0x5a; 32],
        verifying_contract: [0x03; 32],
        block_expiry_height: 1_000,
        nonce: 1,
    };
    // Widest varints: where a client assuming fixed-width integers breaks.
    let extremes = Header {
        network: [0xff; 32],
        verifying_contract: [0xee; 32],
        block_expiry_height: i64::MAX,
        nonce: u64::MAX,
    };

    let vectors = vec![
        vector("grant_zeroed", &zeroed, &Grant { amount: 0 }),
        vector("grant_typical", &typical, &Grant { amount: 500 }),
        // Same header, same body bytes, different payload type. A client
        // producing the same message for these two has no type binding.
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
        // 2: the nonce/expiry pair became a Uniquifier, encoded expiry-first.
        "version": 2,
        "category": "signed_op",
        "domain_separator_cxs_hex": hex(SIGNED_OP.as_bytes()),
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
    fn signed_message_starts_with_separator_and_contains_type_def() {
        let out = generate();
        let sep = out["domain_separator_cxs_hex"].as_str().unwrap();
        assert_eq!(sep, hex(b"\x00\x08SignedOp"));
        for v in out["vectors"].as_array().unwrap() {
            let signed = v["signed_message_hex"].as_str().unwrap();
            assert!(signed.starts_with(sep));
            assert!(signed.contains(v["type_def_cxs_hex"].as_str().unwrap()));
            assert!(signed.ends_with(v["body_cxs_hex"].as_str().unwrap()));
        }
    }

    /// The preimage carries the uniquifier as the chain encodes it: expiry,
    /// then nonce, immediately before the body.
    #[test]
    fn uniquifier_sits_before_the_body_expiry_first() {
        let out = generate();
        let vectors = out["vectors"].as_array().unwrap();
        let v = find(vectors, "grant_typical");
        let signed = v["signed_message_hex"].as_str().unwrap();
        let uniq = v["uniquifier_cxs_hex"].as_str().unwrap();
        let body = v["body_cxs_hex"].as_str().unwrap();
        assert!(signed.ends_with(&format!("{uniq}{body}")));
        let expiry =
            hex(&xdrs::cxs::encode(&BlockHeight::new(1_000)).unwrap());
        assert!(uniq.starts_with(&expiry));
    }

    #[test]
    fn identically_encoded_bodies_get_different_messages() {
        let out = generate();
        let vectors = out["vectors"].as_array().unwrap();
        let grant = find(vectors, "grant_typical");
        let revoke = find(vectors, "revoke_typical");
        assert_eq!(grant["body_cxs_hex"], revoke["body_cxs_hex"]);
        assert_ne!(grant["type_def_cxs_hex"], revoke["type_def_cxs_hex"]);
        assert_ne!(grant["signed_message_hex"], revoke["signed_message_hex"]);
    }
}
