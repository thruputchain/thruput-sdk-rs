use std::{fs, process::Command};

use groundhog_contract_spec::{
    ContractArrayType, ContractField, ContractSpec, ContractSpecMethod,
    ContractSpecString, ContractType, ContractVectorType, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};
use groundhog_spec_tool::{
    FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME,
    rewrite::rewrite_wasm,
};
use serde_json::Value;
use tempfile::tempdir;
use wasm_encoder::{
    ConstExpr, CustomSection, DataSection, MemorySection, MemoryType, Module,
};
use wasmparser::{DataKind, Parser, Payload};
use xdrs::txs::{decode, encode};

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

fn spec_str(value: &ContractSpecString) -> &str {
    std::str::from_utf8(value).unwrap()
}

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

#[test]
fn rewrite_cli_writes_a_json_sidecar() {
    let tempdir = tempdir().unwrap();
    let wasm_path = tempdir.path().join("rewrite-cli.wasm");
    let json_path = tempdir.path().join("rewrite-cli.spec.json");
    let raw = encode_fragments(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("ExplicitNamedInterface"),
            interface_version: s("0.3.0"),
            throws: vec![].try_into().unwrap(),
        },
    }]);

    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
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
fn rewrite_cli_preserves_vec_and_array_of_u8_shapes() {
    let tempdir = tempdir().unwrap();
    let wasm_path = tempdir.path().join("rewrite-cli-u8.wasm");
    let json_path = tempdir.path().join("rewrite-cli-u8.spec.json");
    let raw = encode_fragments(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("ExplicitNamedInterface"),
                interface_version: s("0.3.0"),
                throws: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("ExplicitNamedInterface"),
                interface_version: s("0.3.0"),
                method: ContractSpecMethod {
                    name: s("echo_blob"),
                    args: vec![ContractField {
                        name: s("blob"),
                        ty: ContractType::Vector {
                            vector: ContractVectorType {
                                element: Box::new(ContractType::U8),
                                max_len: u32::MAX,
                            },
                        },
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Vector {
                        vector: ContractVectorType {
                            element: Box::new(ContractType::U8),
                            max_len: u32::MAX,
                        },
                    },
                },
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("ExplicitNamedInterface"),
                interface_version: s("0.3.0"),
                method: ContractSpecMethod {
                    name: s("echo_hash"),
                    args: vec![ContractField {
                        name: s("hash"),
                        ty: ContractType::Array {
                            array: ContractArrayType {
                                element: Box::new(ContractType::U8),
                                len: 32,
                            },
                        },
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Array {
                        array: ContractArrayType {
                            element: Box::new(ContractType::U8),
                            len: 32,
                        },
                    },
                },
            },
        },
    ]);

    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
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
    let methods = &parsed["interfaces"][0]["methods"];
    assert_eq!(methods[0]["name"], "echo_blob");
    assert_eq!(
        methods[0]["args"][0]["ty"],
        serde_json::json!({
            "kind": "vec",
            "inner": { "kind": "u8" },
            "max_len": 4294967295u32
        })
    );
    assert_eq!(
        methods[0]["return_type"],
        serde_json::json!({
            "kind": "vec",
            "inner": { "kind": "u8" },
            "max_len": 4294967295u32
        })
    );
    assert_eq!(methods[1]["name"], "echo_hash");
    assert_eq!(
        methods[1]["args"][0]["ty"],
        serde_json::json!({
            "kind": "array",
            "len": 32,
            "element": { "kind": "u8" }
        })
    );
    assert_eq!(
        methods[1]["return_type"],
        serde_json::json!({
            "kind": "array",
            "len": 32,
            "element": { "kind": "u8" }
        })
    );
}

#[test]
fn rewrite_replaces_raw_section_with_one_final_section() {
    let raw = encode_fragments(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("ExplicitNamedInterface"),
                interface_version: s("0.3.0"),
                throws: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("VersionedCallee"),
                interface_version: s("1.2.3"),
                throws: vec![].try_into().unwrap(),
            },
        },
    ]);
    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
    });

    let rewritten = rewrite_wasm(&module.finish()).unwrap();
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
                spec_str(&interface.name),
                spec_str(&interface.vers)
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
    let raw = encode_fragments(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("ExplicitNamedInterface"),
            interface_version: s("0.3.0"),
            throws: vec![].try_into().unwrap(),
        },
    }]);
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
    let spec: ContractSpec = decode(&final_sections[0].1).unwrap();
    assert_eq!(spec.interfaces.len(), 1);
}

#[test]
fn rewrite_rejects_raw_only_data_segment_leaks() {
    let raw_blob = encode_fragments(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("ExplicitNamedInterface"),
                interface_version: s("0.3.0"),
                throws: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("VersionedCallee"),
                interface_version: s("1.2.3"),
                throws: vec![].try_into().unwrap(),
            },
        },
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

    let err = rewrite_wasm(&module.finish()).unwrap_err();
    assert_eq!(err.to_string(), "raw spec bytes leaked into linear memory");
}

#[test]
fn rewrite_rejects_partial_data_segment_leaks() {
    let leaked_fragment = encode(&SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("ExplicitNamedInterface"),
            interface_version: s("0.3.0"),
            throws: vec![].try_into().unwrap(),
        },
    })
    .unwrap();
    let raw_blob = encode_fragments(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("ExplicitNamedInterface"),
                interface_version: s("0.3.0"),
                throws: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("VersionedCallee"),
                interface_version: s("1.2.3"),
                throws: vec![].try_into().unwrap(),
            },
        },
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

    let err = rewrite_wasm(&module.finish()).unwrap_err();
    assert_eq!(err.to_string(), "raw spec bytes leaked into linear memory");
}

#[test]
fn rewrite_preserves_unrelated_data_segments() {
    let raw_blob = encode_fragments(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("ExplicitNamedInterface"),
            interface_version: s("0.3.0"),
            throws: vec![].try_into().unwrap(),
        },
    }]);
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

    let rewritten = rewrite_wasm(&module.finish()).unwrap();

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
    let raw = encode_fragments(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("ExplicitNamedInterface"),
            interface_version: s("0.3.0"),
            throws: vec![].try_into().unwrap(),
        },
    }]);
    let mut raw_module = Module::new();
    raw_module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
    });

    let first = rewrite_wasm(&raw_module.finish()).unwrap();
    let second = rewrite_wasm(&first).unwrap();
    assert_eq!(final_spec(&first), final_spec(&second));
}

#[test]
fn rewrite_rejects_mixed_data_segment_leaks() {
    let raw_blob = encode_fragments(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("ExplicitNamedInterface"),
                interface_version: s("0.3.0"),
                throws: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("VersionedCallee"),
                interface_version: s("1.2.3"),
                throws: vec![].try_into().unwrap(),
            },
        },
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

    let err = rewrite_wasm(&module.finish()).unwrap_err();
    assert_eq!(err.to_string(), "raw spec bytes leaked into linear memory");
}

#[test]
fn rewrite_omits_final_section_when_raw_has_only_typedefs() {
    let raw_blob = encode_fragments(&[SpecFragment::SpecTypeDef {
        type_def: groundhog_contract_spec::ContractTypeDef::TypeDefStruct {
            struct_def: groundhog_contract_spec::ContractStructDef {
                name: s("HelperType"),
                fields: vec![].try_into().unwrap(),
            },
        },
    }]);

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

    let rewritten = rewrite_wasm(&module.finish()).unwrap();

    assert!(custom_sections(&rewritten).is_empty());
}
