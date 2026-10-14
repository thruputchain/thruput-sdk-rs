use serde_json::Value;
use std::collections::HashSet;

#[test]
fn entrypoint_vectors_valid_json() {
    let output = thruput_test_vectors::entrypoint_vectors::generate();

    // Check top-level structure
    assert_eq!(output["version"], 1);
    assert_eq!(output["category"], "entrypoint_id");

    let vectors = output["vectors"]
        .as_array()
        .expect("vectors should be an array");
    assert!(!vectors.is_empty(), "should have at least one vector");

    // Check each vector has required fields
    for (i, v) in vectors.iter().enumerate() {
        assert!(
            v.get("interface").is_some(),
            "vector {i} missing 'interface'"
        );
        assert!(
            v["version_str"].is_string(),
            "vector {i} missing 'version_str'"
        );
        assert!(
            v["derived_major_version"].is_string(),
            "vector {i} missing 'derived_major_version'"
        );
        assert!(v["method"].is_string(), "vector {i} missing 'method'");
        let id = v["expected_id"]
            .as_str()
            .unwrap_or_else(|| panic!("vector {i} missing 'expected_id'"));
        assert_eq!(
            id.len(),
            16,
            "vector {i} expected_id should be 16 hex chars, got {}",
            id.len()
        );
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase()),
            "vector {i} expected_id should be uppercase hex"
        );
    }

    // Check vector keys are unique (interface + version_str + method)
    let keys: HashSet<String> = vectors
        .iter()
        .map(|v| {
            format!("{}:{}:{}", v["interface"], v["version_str"], v["method"])
        })
        .collect();
    assert_eq!(
        keys.len(),
        vectors.len(),
        "entrypoint vector keys must be unique"
    );
}

#[test]
fn xdr_vectors_valid_json() {
    let output = thruput_test_vectors::xdr_vectors::generate();

    // Check top-level structure
    assert_eq!(output["version"], 1);
    assert_eq!(output["category"], "xdr_serialization");

    let vectors = output["vectors"]
        .as_array()
        .expect("vectors should be an array");
    assert!(!vectors.is_empty(), "should have at least one vector");

    // Check each vector has required fields
    for (i, v) in vectors.iter().enumerate() {
        assert!(v["name"].is_string(), "vector {i} missing 'name'");
        assert!(v.get("type").is_some(), "vector {i} missing 'type'");
        assert!(v.get("value").is_some(), "vector {i} missing 'value'");
        let hex = v["expected_hex"]
            .as_str()
            .unwrap_or_else(|| panic!("vector {i} missing 'expected_hex'"));
        assert!(
            hex.chars().all(|c| c.is_ascii_hexdigit()),
            "vector {i} expected_hex should be hex"
        );
        assert!(
            hex.len() % 2 == 0,
            "vector {i} expected_hex should have even length"
        );
    }

    // Check vector names are unique
    let names: HashSet<&str> =
        vectors.iter().filter_map(|v| v["name"].as_str()).collect();
    assert_eq!(
        names.len(),
        vectors.len(),
        "xdr vector names must be unique"
    );
}

#[test]
fn entrypoint_vectors_deterministic() {
    let a = thruput_test_vectors::entrypoint_vectors::generate();
    let b = thruput_test_vectors::entrypoint_vectors::generate();
    assert_eq!(a, b, "entrypoint vectors should be deterministic");
}

#[test]
fn xdr_vectors_deterministic() {
    let a = thruput_test_vectors::xdr_vectors::generate();
    let b = thruput_test_vectors::xdr_vectors::generate();
    assert_eq!(a, b, "xdr vectors should be deterministic");
}

#[test]
fn entrypoint_vectors_roundtrip_json_string() {
    let output = thruput_test_vectors::entrypoint_vectors::generate();
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    let parsed: Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(
        output, parsed,
        "roundtrip through JSON string should be lossless"
    );
}

#[test]
fn xdr_vectors_roundtrip_json_string() {
    let output = thruput_test_vectors::xdr_vectors::generate();
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    let parsed: Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(
        output, parsed,
        "roundtrip through JSON string should be lossless"
    );
}

#[test]
fn pubkey_auth_vectors_deterministic() {
    let a = thruput_test_vectors::pubkey_auth_vectors::generate();
    let b = thruput_test_vectors::pubkey_auth_vectors::generate();
    assert_eq!(a, b, "pubkey-auth vectors should be deterministic");
}

/// Every stage of every vector is present and the sizes are the scheme's,
/// and each stage is built from the one before it: the preimage ends with
/// the transaction, and the signed transaction starts with it.
#[test]
fn pubkey_auth_vectors_valid_json() {
    let output = thruput_test_vectors::pubkey_auth_vectors::generate();
    assert_eq!(output["version"], 1);
    assert_eq!(output["category"], "pubkey_auth");
    let separator = output["transaction_domain_separator_cxs_hex"]
        .as_str()
        .unwrap();
    let vectors = output["vectors"].as_array().unwrap();
    assert!(vectors.iter().any(|v| v["scheme"] == "Ed25519"));
    assert!(vectors.iter().any(|v| v["scheme"] == "MlDsa65"));

    for v in vectors {
        let name = v["name"].as_str().unwrap();
        let field = |k: &str| {
            v[k].as_str()
                .unwrap_or_else(|| panic!("{name}: missing '{k}'"))
                .to_string()
        };
        let (key_len, sig_len) = match v["scheme"].as_str().unwrap() {
            "Ed25519" => (32, 64),
            "MlDsa65" => (1952, 3309),
            s => panic!("{name}: unexpected scheme {s}"),
        };
        assert_eq!(field("seed_hex").len(), 64, "{name}");
        assert_eq!(field("public_key_hex").len(), 2 * key_len, "{name}");
        assert_eq!(
            field("public_key_cxs_hex").len(),
            2 * (1 + key_len),
            "{name}"
        );
        assert_eq!(field("public_key_hash_hex").len(), 64, "{name}");
        assert_eq!(field("address_hex").len(), 64, "{name}");
        assert_eq!(field("transaction_hash_hex").len(), 64, "{name}");
        assert_eq!(field("signature_hex").len(), 2 * sig_len, "{name}");

        let tx = field("transaction_xdr_hex");
        let preimage = field("signing_preimage_hex");
        assert_eq!(
            preimage,
            format!("{separator}{}{tx}", field("network_hash_hex")),
            "{name}: preimage"
        );
        assert!(
            field("signed_transaction_xdr_hex").starts_with(&tx),
            "{name}"
        );
        assert_eq!(
            v["transaction"]["invoked_address_hex"], v["address_hex"],
            "{name}"
        );
    }
}
