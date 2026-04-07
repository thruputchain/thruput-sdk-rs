use std::{fs, path::PathBuf, process::Command};

#[test]
fn built_spec_includes_named_type_defs() {
    let sdk_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("example crate lives under example-contracts")
        .to_path_buf();
    let status = Command::new("bash")
        .current_dir(&sdk_dir)
        .args([
            "-lc",
            "source scripts/dev.sh && eg_build spec_named_types",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "failed building spec_named_types");

    let json = sdk_dir.join("target/optimized/spec_named_types.spec.json");
    let text = fs::read_to_string(json).unwrap();

    assert!(text.contains("\"type_defs\""));
    assert!(text.contains("\"SpecNamedTypesContract\""));
    assert!(text.contains("\"Payload\""));
    assert!(text.contains("\"Choice\""));
    assert!(text.contains("\"Envelope\""));
}
