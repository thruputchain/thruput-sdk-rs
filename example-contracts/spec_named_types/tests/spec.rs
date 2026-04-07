use std::{fs, path::PathBuf};

#[test]
fn built_spec_includes_named_type_defs() {
    let sdk_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("example crate lives under example-contracts")
        .to_path_buf();
    let json = sdk_dir.join("target/optimized/spec_named_types.spec.json");
    let text = fs::read_to_string(json).unwrap();

    assert!(text.contains("\"type_defs\""));
    assert!(text.contains("\"Payload\""));
    assert!(text.contains("\"Choice\""));
    assert!(text.contains("\"Envelope\""));
}
