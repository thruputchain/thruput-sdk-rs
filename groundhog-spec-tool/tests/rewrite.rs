use std::{fs, process::Command};

use groundhog_spec_tool::{rewrite::rewrite_wasm, FINAL_SECTION_NAME, RAW_SECTION_NAME};
use serde_json::Value;
use tempfile::tempdir;
use wasm_encoder::{CustomSection, Module};
use wasmparser::{Parser, Payload};

fn custom_sections(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut sections = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CustomSection(section) = payload.unwrap() {
            sections.push((section.name().to_owned(), section.data().to_vec()));
        }
    }
    sections
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
