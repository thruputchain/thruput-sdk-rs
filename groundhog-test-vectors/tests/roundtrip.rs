use serde_json::Value;
use std::collections::HashSet;

#[test]
fn entrypoint_vectors_valid_json() {
    let output = groundhog_test_vectors::entrypoint_vectors::generate();

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
            id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase()),
            "vector {i} expected_id should be uppercase hex"
        );
    }

    // Check vector keys are unique (interface + version_str + method)
    let keys: HashSet<String> = vectors
        .iter()
        .map(|v| {
            format!(
                "{}:{}:{}",
                v["interface"], v["version_str"], v["method"]
            )
        })
        .collect();
    assert_eq!(keys.len(), vectors.len(), "entrypoint vector keys must be unique");
}

#[test]
fn xdr_vectors_valid_json() {
    let output = groundhog_test_vectors::xdr_vectors::generate();

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
    let names: HashSet<&str> = vectors
        .iter()
        .filter_map(|v| v["name"].as_str())
        .collect();
    assert_eq!(names.len(), vectors.len(), "xdr vector names must be unique");
}

#[test]
fn entrypoint_vectors_deterministic() {
    let a = groundhog_test_vectors::entrypoint_vectors::generate();
    let b = groundhog_test_vectors::entrypoint_vectors::generate();
    assert_eq!(a, b, "entrypoint vectors should be deterministic");
}

#[test]
fn xdr_vectors_deterministic() {
    let a = groundhog_test_vectors::xdr_vectors::generate();
    let b = groundhog_test_vectors::xdr_vectors::generate();
    assert_eq!(a, b, "xdr vectors should be deterministic");
}

#[test]
fn entrypoint_vectors_roundtrip_json_string() {
    let output = groundhog_test_vectors::entrypoint_vectors::generate();
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    let parsed: Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(
        output, parsed,
        "roundtrip through JSON string should be lossless"
    );
}

#[test]
fn xdr_vectors_roundtrip_json_string() {
    let output = groundhog_test_vectors::xdr_vectors::generate();
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    let parsed: Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(
        output, parsed,
        "roundtrip through JSON string should be lossless"
    );
}
