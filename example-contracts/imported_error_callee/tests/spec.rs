use std::{fs, path::PathBuf, process::Command};

#[test]
fn built_spec_includes_imported_throw_type_def() {
    let sdk_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("example crate lives under example-contracts")
        .to_path_buf();
    let status = Command::new("bash")
        .current_dir(&sdk_dir)
        .args([
            "-lc",
            "source scripts/dev.sh && eg_build imported_error_callee",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "failed building imported_error_callee");

    let json = sdk_dir.join("target/optimized/imported_error_callee.spec.json");
    let text = fs::read_to_string(json).unwrap();

    assert!(text.contains("\"ImportedErrorCallee\""));
    assert!(text.contains("\"ImportedError\""));
}
