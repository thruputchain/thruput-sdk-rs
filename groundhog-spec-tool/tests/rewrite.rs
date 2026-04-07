use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Mutex, OnceLock},
};

use groundhog_contract_spec::{
    ContractSpec, SpecFragment, SpecInterfaceFragment,
};
use groundhog_spec_tool::{
    FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME,
    parse_raw_fragments, rewrite::rewrite_wasm,
};
use serde_json::Value;
use tempfile::tempdir;
use wasm_encoder::{
    ConstExpr, CustomSection, DataSection, MemorySection, MemoryType, Module,
};
use wasmparser::{DataKind, Parser, Payload};
use xdrs::txs::{decode, encode};

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

fn data_segments(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut segments = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::DataSection(reader) = payload.unwrap() {
            for segment in reader {
                let segment = segment.unwrap();
                if matches!(segment.kind, DataKind::Active { .. }) {
                    segments.push(segment.data.to_vec());
                }
            }
        }
    }
    segments
}

fn export_names(bytes: &[u8]) -> Vec<String> {
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::ExportSection(reader) = payload.unwrap() {
            for export in reader {
                exports.push(export.unwrap().name.to_owned());
            }
        }
    }
    exports
}

fn encode_fragments(fragments: &[SpecFragment]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for fragment in fragments {
        bytes.extend(encode(fragment).unwrap());
    }
    bytes
}

fn final_spec(bytes: &[u8]) -> Option<ContractSpec> {
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CustomSection(section) = payload.unwrap() {
            if section.name() == FINAL_SECTION_NAME {
                return Some(decode(section.data()).unwrap());
            }
        }
    }
    None
}

fn raw_fragments(bytes: &[u8]) -> Vec<SpecFragment> {
    let mut fragments = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CustomSection(section) = payload.unwrap() {
            if section.name() == RAW_SECTION_NAME {
                fragments.extend(parse_raw_fragments(section.data()).unwrap());
            }
        }
    }
    fragments
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("groundhog-spec-tool lives in the workspace")
        .to_path_buf()
}

fn example_build_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn build_example_contract(package: &str) {
    let _guard = example_build_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
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
    let raw =
        encode_fragments(&[SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        })]);

    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
    });
    fs::write(&wasm_path, module.finish()).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_groundhog-spec-tool"))
        .args([
            "rewrite",
            "--package",
            "demo",
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
    assert_eq!(parsed["format"], FINAL_FORMAT_TAG);
    assert_eq!(
        parsed["interfaces"],
        serde_json::json!([{
            "name": "ExplicitNamedInterface",
            "version": "0.3.0",
            "throws": [],
            "methods": []
        }])
    );
    assert_eq!(parsed["type_defs"], serde_json::json!([]));
}

#[test]
fn rewrite_replaces_raw_section_with_one_final_section() {
    let raw = encode_fragments(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        }),
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "VersionedCallee".into(),
            interface_version: "1.2.3".into(),
            throws: vec![],
        }),
    ]);
    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
    });

    let rewritten = rewrite_wasm(&module.finish(), "demo").unwrap();
    let sections = custom_sections(&rewritten);

    assert!(sections.iter().all(|(name, _)| name != RAW_SECTION_NAME));
    let final_sections: Vec<_> = sections
        .into_iter()
        .filter(|(name, _)| name == FINAL_SECTION_NAME)
        .collect();
    assert_eq!(final_sections.len(), 1);
    let spec: ContractSpec = decode(&final_sections[0].1).unwrap();
    assert_eq!(
        spec.interfaces
            .iter()
            .map(|interface| (
                interface.name.as_str(),
                interface.version.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("ExplicitNamedInterface", "0.3.0"),
            ("VersionedCallee", "1.2.3"),
        ]
    );
}

#[test]
fn rewrite_preserves_unrelated_custom_sections() {
    let raw =
        encode_fragments(&[SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        })]);
    let mut module = Module::new();
    module.section(&CustomSection {
        name: "kept".into(),
        data: b"retain-me".as_slice().into(),
    });
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
    });
    module.section(&CustomSection {
        name: "also-kept".into(),
        data: b"retain-me-too".as_slice().into(),
    });
    module.section(&CustomSection {
        name: FINAL_SECTION_NAME.into(),
        data: b"stale\n".as_slice().into(),
    });

    let rewritten = rewrite_wasm(&module.finish(), "demo").unwrap();
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
    let spec: ContractSpec = decode(&final_sections[0].1).unwrap();
    assert_eq!(spec.interfaces.len(), 1);
}

#[test]
fn rewrite_rejects_raw_only_data_segment_leaks() {
    let raw_blob = encode_fragments(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        }),
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "VersionedCallee".into(),
            interface_version: "1.2.3".into(),
            throws: vec![],
        }),
    ]);

    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut data = DataSection::new();
    data.active(0, &ConstExpr::i32_const(0), raw_blob.iter().copied());

    let mut module = Module::new();
    module.section(&memory);
    module.section(&data);
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw_blob.as_slice().into(),
    });

    let err = rewrite_wasm(&module.finish(), "demo").unwrap_err();
    assert_eq!(err.to_string(), "raw spec bytes leaked into linear memory");
}

#[test]
fn rewrite_rejects_partial_data_segment_leaks() {
    let leaked_fragment =
        encode(&SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        }))
        .unwrap();
    let raw_blob = encode_fragments(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        }),
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "VersionedCallee".into(),
            interface_version: "1.2.3".into(),
            throws: vec![],
        }),
    ]);

    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut data = DataSection::new();
    data.active(0, &ConstExpr::i32_const(0), leaked_fragment.iter().copied());

    let mut module = Module::new();
    module.section(&memory);
    module.section(&data);
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw_blob.as_slice().into(),
    });

    let err = rewrite_wasm(&module.finish(), "demo").unwrap_err();
    assert_eq!(err.to_string(), "raw spec bytes leaked into linear memory");
}

#[test]
fn rewrite_preserves_unrelated_data_segments() {
    let raw_blob =
        encode_fragments(&[SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        })]);
    let kept_data = b"retain-me".to_vec();

    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut data = DataSection::new();
    data.active(0, &ConstExpr::i32_const(0), kept_data.iter().copied());

    let mut module = Module::new();
    module.section(&memory);
    module.section(&data);
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw_blob.as_slice().into(),
    });

    let rewritten = rewrite_wasm(&module.finish(), "demo").unwrap();

    assert_eq!(data_segments(&rewritten), vec![kept_data]);
    assert_eq!(
        custom_sections(&rewritten)
            .into_iter()
            .filter(|(name, _)| name == FINAL_SECTION_NAME)
            .count(),
        1
    );
}

#[test]
fn rewrite_preserves_final_section_when_rerun_without_raw_fragments() {
    let raw =
        encode_fragments(&[SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        })]);
    let mut raw_module = Module::new();
    raw_module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
    });

    let first = rewrite_wasm(&raw_module.finish(), "demo").unwrap();
    let second = rewrite_wasm(&first, "demo").unwrap();
    assert_eq!(final_spec(&first), final_spec(&second));
}

#[test]
fn rewrite_rejects_mixed_data_segment_leaks() {
    let raw_blob = encode_fragments(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        }),
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "VersionedCallee".into(),
            interface_version: "1.2.3".into(),
            throws: vec![],
        }),
    ]);
    let retained_suffix = [0_u8, 0, 0, 0, b'K', b'E', b'P', b'T'];
    let original_offset = 7;
    let mut mixed_data = raw_blob.clone();
    mixed_data.extend_from_slice(&retained_suffix);

    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut data = DataSection::new();
    data.active(
        0,
        &ConstExpr::i32_const(original_offset),
        mixed_data.iter().copied(),
    );

    let mut module = Module::new();
    module.section(&memory);
    module.section(&data);
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw_blob.as_slice().into(),
    });

    let err = rewrite_wasm(&module.finish(), "demo").unwrap_err();
    assert_eq!(err.to_string(), "raw spec bytes leaked into linear memory");
}

fn assert_built_contract_wasm_strips_staging_bytes_and_anchor_export(
    package: &str,
) {
    build_example_contract(package);

    let wasm_path = workspace_root()
        .join("target/optimized")
        .join(format!("{}.wasm", package.replace('-', "_")));
    let wasm = fs::read(&wasm_path).unwrap();
    let sections = custom_sections(&wasm);

    assert_eq!(
        sections
            .iter()
            .filter(|(name, _)| name == FINAL_SECTION_NAME)
            .count(),
        1
    );
    assert!(sections.iter().all(|(name, _)| name != RAW_SECTION_NAME));
    assert!(!export_names(&wasm).iter().any(|name| name == "_"));
}

#[test]
fn built_contract_wasm_strips_raw_staging_bytes_and_anchor_export() {
    assert_built_contract_wasm_strips_staging_bytes_and_anchor_export(
        "free_fn_unnamed",
    );
    assert_built_contract_wasm_strips_staging_bytes_and_anchor_export(
        "multi_imported_traits",
    );
}

#[test]
fn raw_release_contract_keeps_spec_bytes_out_of_linear_memory() {
    build_example_contract("free_fn_unnamed");

    let wasm_path = workspace_root()
        .join("target/wasm32-unknown-unknown/release/free_fn_unnamed.wasm");
    let wasm = fs::read(&wasm_path).unwrap();
    let fragments = raw_fragments(&wasm);

    assert!(
        custom_sections(&wasm)
            .iter()
            .any(|(name, _)| name == RAW_SECTION_NAME)
    );
    assert!(!fragments.is_empty());
    assert!(
        data_segments(&wasm)
            .iter()
            .all(|segment| fragments.iter().all(|fragment| {
                let encoded = encode(fragment).unwrap();
                !contains_bytes(segment, &encoded)
            }))
    );
}

#[test]
fn built_spec_complete_contracts_produce_expected_final_interface_lists() {
    let cases = [
        (
            "multi_imported_traits",
            vec![
                ("ExplicitNamedInterface", "0.3.0"),
                ("VersionedCallee", "1.2.3"),
            ],
        ),
        ("free_fn_unnamed", vec![("_unnamed_interface", "0.3.0")]),
        (
            "spec_named_types",
            vec![("SpecNamedTypesContract", "0.3.0")],
        ),
    ];

    for (package, expected_interfaces) in cases {
        let parsed = read_sidecar_interfaces(package);
        assert_eq!(parsed["format"], FINAL_FORMAT_TAG);
        let interfaces = parsed["interfaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|interface| {
                (
                    interface["name"].as_str().unwrap().to_owned(),
                    interface["version"].as_str().unwrap().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            interfaces,
            expected_interfaces
                .into_iter()
                .map(|(name, version)| (name.to_owned(), version.to_owned()))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn rewrite_filters_fragments_to_requested_package() {
    let mixed_raw_blob = encode_fragments(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "helper".into(),
            interface_name: "LeakedInterface".into(),
            interface_version: "9.9.9".into(),
            throws: vec![],
        }),
        SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "owner".into(),
            interface_name: "ExplicitNamedInterface".into(),
            interface_version: "0.3.0".into(),
            throws: vec![],
        }),
    ]);

    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: mixed_raw_blob.as_slice().into(),
    });

    let rewritten = rewrite_wasm(&module.finish(), "owner").unwrap();
    let sections = custom_sections(&rewritten);
    let final_section = sections
        .into_iter()
        .find(|(name, _)| name == FINAL_SECTION_NAME)
        .unwrap();
    let spec: ContractSpec = decode(&final_section.1).unwrap();
    assert_eq!(
        spec.interfaces
            .iter()
            .map(|interface| (
                interface.name.as_str(),
                interface.version.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![("ExplicitNamedInterface", "0.3.0")]
    );
}

#[test]
fn rewrite_omits_final_section_when_requested_package_has_no_fragments() {
    let raw_blob =
        encode_fragments(&[SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "helper".into(),
            interface_name: "LeakedInterface".into(),
            interface_version: "9.9.9".into(),
            throws: vec![],
        })]);

    let mut module = Module::new();
    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memory);
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw_blob.as_slice().into(),
    });

    let rewritten = rewrite_wasm(&module.finish(), "owner").unwrap();

    assert!(custom_sections(&rewritten).is_empty());
}

#[test]
fn built_noop_contract_does_not_emit_spec_sidecar() {
    build_example_contract("genesis_deployer");

    let wasm_path =
        workspace_root().join("target/optimized/genesis_deployer.wasm");
    let json_path =
        workspace_root().join("target/optimized/genesis_deployer.spec.json");
    let wasm = fs::read(&wasm_path).unwrap();

    assert!(wasm_path.is_file());
    assert!(!json_path.exists());
    assert!(custom_sections(&wasm).is_empty());
}
