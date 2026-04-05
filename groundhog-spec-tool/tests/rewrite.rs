use std::{fs, path::PathBuf, process::Command};

use groundhog_spec_tool::{
    FINAL_SECTION_NAME, RAW_SECTION_NAME, rewrite::rewrite_wasm,
};
use serde_json::Value;
use tempfile::tempdir;
use wasm_encoder::{CustomSection, Module};
use wasmparser::{Parser, Payload};

fn custom_sections(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut sections = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CustomSection(section) = payload.unwrap() {
            sections
                .push((section.name().to_owned(), section.data().to_vec()));
        }
    }
    sections
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("groundhog-spec-tool lives in the workspace")
        .to_path_buf()
}

fn build_example_contract(package: &str) {
    let status = Command::new("bash")
        .current_dir(workspace_root())
        .args([
            "-lc",
            &format!("source scripts/dev.sh && eg_build {package}"),
        ])
        .status()
        .unwrap();

    assert!(status.success(), "failed building {package}");
}

fn read_sidecar_interfaces(package: &str) -> Value {
    build_example_contract(package);

    let wasm_name = package.replace('-', "_");
    let json_path = workspace_root()
        .join("target/optimized")
        .join(format!("{wasm_name}.spec.json"));
    let json = fs::read_to_string(&json_path).unwrap();
    serde_json::from_str(&json).unwrap()
}

#[test]
fn rewrite_cli_writes_a_json_sidecar() {
    let tempdir = tempdir().unwrap();
    let wasm_path = tempdir.path().join("rewrite-cli.wasm");
    let json_path = tempdir.path().join("rewrite-cli.spec.json");

    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: b"iface\tdemo\tAlpha\tExplicitNamedInterface\t0.3.0\n"
            .as_slice()
            .into(),
    });
    fs::write(&wasm_path, module.finish()).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_groundhog-spec-tool"))
        .args([
            "rewrite",
            "--wasm",
            wasm_path.to_str().unwrap(),
            "--json-out",
            json_path.to_str().unwrap(),
        ])
        .status()
        .unwrap();

    assert!(status.success());

    let json = fs::read_to_string(&json_path).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["format"], "gh_iface_v1");
    assert_eq!(
        parsed["interfaces"],
        serde_json::json!([{
            "interface_name": "ExplicitNamedInterface",
            "interface_version": "0.3.0"
        }])
    );
}

#[test]
fn rewrite_replaces_raw_section_with_one_final_section() {
    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: b"iface\tdemo\tAlpha\tExplicitNamedInterface\t0.3.0\n\
                iface\tdemo\tAlpha\tVersionedCallee\t1.2.3\n"
            .as_slice()
            .into(),
    });

    let rewritten = rewrite_wasm(&module.finish()).unwrap();
    let sections = custom_sections(&rewritten);

    assert!(sections.iter().all(|(name, _)| name != RAW_SECTION_NAME));
    let final_sections: Vec<_> = sections
        .into_iter()
        .filter(|(name, _)| name == FINAL_SECTION_NAME)
        .collect();
    assert_eq!(final_sections.len(), 1);
    let body = String::from_utf8(final_sections[0].1.clone()).unwrap();
    assert!(body.contains("gh_iface_v1"));
    assert!(body.contains("ExplicitNamedInterface\t0.3.0"));
    assert!(body.contains("VersionedCallee\t1.2.3"));
}

#[test]
fn rewrite_preserves_unrelated_custom_sections() {
    let mut module = Module::new();
    module.section(&CustomSection {
        name: "kept".into(),
        data: b"retain-me".as_slice().into(),
    });
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: b"iface\tdemo\tAlpha\tExplicitNamedInterface\t0.3.0\n"
            .as_slice()
            .into(),
    });
    module.section(&CustomSection {
        name: "also-kept".into(),
        data: b"retain-me-too".as_slice().into(),
    });
    module.section(&CustomSection {
        name: FINAL_SECTION_NAME.into(),
        data: b"stale\n".as_slice().into(),
    });

    let rewritten = rewrite_wasm(&module.finish()).unwrap();
    let sections = custom_sections(&rewritten);

    assert!(sections.iter().any(|(name, body)| {
        name == "kept" && body.as_slice() == b"retain-me"
    }));
    assert!(sections.iter().any(|(name, body)| {
        name == "also-kept" && body.as_slice() == b"retain-me-too"
    }));
    assert!(sections.iter().all(|(name, _)| name != RAW_SECTION_NAME));
    let final_sections: Vec<_> = sections
        .iter()
        .filter(|(name, _)| name == FINAL_SECTION_NAME)
        .collect();
    assert_eq!(final_sections.len(), 1);
    assert_ne!(final_sections[0].1.as_slice(), b"stale\n");
}

#[test]
fn built_contracts_produce_expected_final_interface_lists() {
    let cases = [
        (
            "counter",
            serde_json::json!([{
                "interface_name": "Counter",
                "interface_version": "0.3.0"
            }]),
        ),
        (
            "erc20_low_level",
            serde_json::json!([{
                "interface_name": "ERC20",
                "interface_version": "0.1.0"
            }]),
        ),
        (
            "multi_imported_traits",
            serde_json::json!([
                {
                    "interface_name": "ExplicitNamedInterface",
                    "interface_version": "0.3.0"
                },
                {
                    "interface_name": "VersionedCallee",
                    "interface_version": "1.2.3"
                }
            ]),
        ),
        (
            "free_fn_unnamed",
            serde_json::json!([{
                "interface_name": "_unnamed_interface",
                "interface_version": "0.3.0"
            }]),
        ),
    ];

    for (package, expected_interfaces) in cases {
        let parsed = read_sidecar_interfaces(package);
        assert_eq!(parsed["format"], "gh_iface_v1");
        assert_eq!(parsed["interfaces"], expected_interfaces);
    }
}
