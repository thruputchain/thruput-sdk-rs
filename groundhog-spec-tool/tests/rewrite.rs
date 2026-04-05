use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Mutex, OnceLock},
};

use groundhog_spec_tool::{
    FINAL_SECTION_NAME, RAW_SECTION_NAME, RawInterfaceFragment,
    encode_raw_fragment, rewrite::rewrite_wasm,
};
use serde_json::Value;
use tempfile::tempdir;
use wasm_encoder::{
    ConstExpr, CustomSection, DataSection, MemorySection, MemoryType, Module,
};
use wasmparser::{DataKind, Operator, Parser, Payload};

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

fn active_data_segments(bytes: &[u8]) -> Vec<(u32, i32, Vec<u8>)> {
    let mut segments = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::DataSection(reader) = payload.unwrap() {
            for segment in reader {
                let segment = segment.unwrap();
                if let DataKind::Active {
                    memory_index,
                    offset_expr,
                } = segment.kind
                {
                    let mut ops = offset_expr.get_operators_reader();
                    let offset = match ops.read().unwrap() {
                        Operator::I32Const { value } => value,
                        other => panic!(
                            "unexpected offset expr operator: {other:?}"
                        ),
                    };
                    match ops.read().unwrap() {
                        Operator::End => {}
                        other => panic!(
                            "unexpected trailing offset expr operator: {other:?}"
                        ),
                    }
                    segments.push((
                        memory_index,
                        offset,
                        segment.data.to_vec(),
                    ));
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
    let _guard = example_build_lock().lock().unwrap();
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

    let rewritten = rewrite_wasm(&module.finish(), "demo").unwrap();
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
    assert_ne!(final_sections[0].1.as_slice(), b"stale\n");
}

#[test]
fn rewrite_drops_raw_only_data_segment_and_keeps_final_section() {
    let raw_fragments = [
        RawInterfaceFragment::impl_backed(
            "demo",
            "Alpha",
            "ExplicitNamedInterface",
            "0.3.0",
        ),
        RawInterfaceFragment::impl_backed(
            "demo",
            "Alpha",
            "VersionedCallee",
            "1.2.3",
        ),
    ];
    let raw_blob = raw_fragments
        .iter()
        .flat_map(encode_raw_fragment)
        .collect::<Vec<_>>();

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

    let rewritten = rewrite_wasm(&module.finish(), "demo").unwrap();
    let sections = custom_sections(&rewritten);

    assert!(sections.iter().all(|(name, _)| name != RAW_SECTION_NAME));
    assert_eq!(
        sections
            .iter()
            .filter(|(name, _)| name == FINAL_SECTION_NAME)
            .count(),
        1
    );
    assert!(data_segments(&rewritten).is_empty());
}

#[test]
fn rewrite_preserves_unrelated_data_segments() {
    let raw_blob = encode_raw_fragment(&RawInterfaceFragment::impl_backed(
        "demo",
        "Alpha",
        "ExplicitNamedInterface",
        "0.3.0",
    ));
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
fn rewrite_rebases_mixed_data_segment_after_stripping_raw_prefix() {
    let raw_fragments = [
        RawInterfaceFragment::impl_backed(
            "demo",
            "Alpha",
            "ExplicitNamedInterface",
            "0.3.0",
        ),
        RawInterfaceFragment::impl_backed(
            "demo",
            "Alpha",
            "VersionedCallee",
            "1.2.3",
        ),
    ];
    let raw_blob = raw_fragments
        .iter()
        .flat_map(encode_raw_fragment)
        .collect::<Vec<_>>();
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

    let rewritten = rewrite_wasm(&module.finish(), "demo").unwrap();
    let active_segments = active_data_segments(&rewritten);

    assert_eq!(
        active_segments,
        vec![(
            0,
            original_offset + raw_blob.len() as i32,
            retained_suffix.to_vec(),
        )]
    );
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
    assert!(
        data_segments(&wasm)
            .iter()
            .all(|segment| !contains_bytes(segment, b"iface\t"))
    );
    assert!(!contains_bytes(&wasm, b"iface\t"));
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

#[test]
fn rewrite_filters_fragments_to_requested_package() {
    let owner_fragment = RawInterfaceFragment::impl_backed(
        "owner",
        "Alpha",
        "ExplicitNamedInterface",
        "0.3.0",
    );
    let helper_fragment = RawInterfaceFragment::impl_backed(
        "helper",
        "Helper",
        "LeakedInterface",
        "9.9.9",
    );
    let mut data = DataSection::new();
    let mixed_raw_blob = [helper_fragment.clone(), owner_fragment.clone()]
        .into_iter()
        .flat_map(|fragment| encode_raw_fragment(&fragment))
        .collect::<Vec<_>>();
    data.active(
        0,
        &ConstExpr::i32_const(0),
        mixed_raw_blob.iter().copied(),
    );

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
    module.section(&data);
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
    let body = String::from_utf8(final_section.1).unwrap();

    assert!(body.contains("ExplicitNamedInterface\t0.3.0"));
    assert!(!body.contains("LeakedInterface"));
    assert!(data_segments(&rewritten).is_empty());
    assert!(!contains_bytes(&rewritten, b"iface\t"));
}

#[test]
fn rewrite_omits_final_section_when_requested_package_has_no_fragments() {
    let helper_fragment = RawInterfaceFragment::impl_backed(
        "helper",
        "Helper",
        "LeakedInterface",
        "9.9.9",
    );
    let raw_blob = encode_raw_fragment(&helper_fragment);

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

    let wasm_path = workspace_root()
        .join("target/optimized/genesis_deployer.wasm");
    let json_path = workspace_root()
        .join("target/optimized/genesis_deployer.spec.json");
    let wasm = fs::read(&wasm_path).unwrap();

    assert!(wasm_path.is_file());
    assert!(!json_path.exists());
    assert!(custom_sections(&wasm).is_empty());
}
