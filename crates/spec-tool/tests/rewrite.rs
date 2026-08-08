use std::{fs, process::Command};

use groundhog_contract_spec::{
    ContractArrayType, ContractEventDef, ContractField, ContractSpec,
    ContractSpecMethod, ContractSpecString, ContractType, ContractVectorType,
    SpecFragment, SpecInterfaceAnchor, SpecInterfaceFragment,
    SpecMethodFragment,
};
use groundhog_spec_tool::{
    FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME, SpecToolError,
    rewrite::rewrite_wasm,
};
use serde_json::Value;
use tempfile::tempdir;
use wasm_encoder::{
    CodeSection, ConstExpr, CustomSection, DataSection, ExportKind,
    ExportSection, Function, FunctionSection, Instruction, MemorySection,
    MemoryType, Module, TypeSection,
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
    // Anchor every interface the fragments mention — what a real `#[contract]
    // impl` emits, and what spec shaking requires for the interface to survive.
    let mut keys = std::collections::BTreeSet::new();
    for fragment in fragments {
        match fragment {
            SpecFragment::SpecInterface {
                interface_fragment: i,
            } => {
                keys.insert((
                    i.interface_name.clone(),
                    i.interface_version.clone(),
                ));
            }
            SpecFragment::SpecMethod { method_fragment: m } => {
                keys.insert((
                    m.interface_name.clone(),
                    m.interface_version.clone(),
                ));
            }
            _ => {}
        }
    }
    for (interface_name, interface_version) in keys {
        bytes.extend(
            encode(&SpecFragment::InterfaceAnchor {
                interface_anchor: SpecInterfaceAnchor {
                    interface_name,
                    interface_version,
                },
            })
            .unwrap(),
        );
    }
    bytes
}

fn final_spec(bytes: &[u8]) -> Option<ContractSpec> {
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CustomSection(section) = payload.unwrap()
            && section.name() == FINAL_SECTION_NAME
        {
            return Some(decode(section.data()).unwrap());
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
            emits: vec![].try_into().unwrap(),
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
                emits: vec![].try_into().unwrap(),
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
        methods[0]["args"][0]["type"],
        serde_json::json!({
            "kind": "vec",
            "element": { "kind": "u8" },
            "max_len": 4294967295u32
        })
    );
    assert_eq!(
        methods[0]["return_type"],
        serde_json::json!({
            "kind": "vec",
            "element": { "kind": "u8" },
            "max_len": 4294967295u32
        })
    );
    assert_eq!(methods[1]["name"], "echo_hash");
    assert_eq!(
        methods[1]["args"][0]["type"],
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
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("VersionedCallee"),
                interface_version: s("1.2.3"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
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
            emits: vec![].try_into().unwrap(),
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
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("VersionedCallee"),
                interface_version: s("1.2.3"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
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
            emits: vec![].try_into().unwrap(),
        },
    })
    .unwrap();
    let raw_blob = encode_fragments(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("ExplicitNamedInterface"),
                interface_version: s("0.3.0"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("VersionedCallee"),
                interface_version: s("1.2.3"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
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
            emits: vec![].try_into().unwrap(),
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
            emits: vec![].try_into().unwrap(),
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
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("VersionedCallee"),
                interface_version: s("1.2.3"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
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

fn typedef_only_module(with_function_export: bool) -> Vec<u8> {
    raw_fragment_module_bool(
        encode_fragments(&[SpecFragment::SpecTypeDef {
            type_def:
                groundhog_contract_spec::ContractTypeDef::TypeDefStruct {
                    struct_def: groundhog_contract_spec::ContractStructDef {
                        name: s("HelperType"),
                        fields: vec![].try_into().unwrap(),
                    },
                },
        }]),
        with_function_export,
    )
}

/// Like [`encode_fragments`] but WITHOUT the auto-anchoring — for fixtures
/// modeling interface candidates that nothing in the artifact implements.
fn encode_fragments_unanchored(fragments: &[SpecFragment]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for fragment in fragments {
        bytes.extend(encode(fragment).unwrap());
    }
    bytes
}

/// Module whose raw section carries an unanchored interface candidate (with
/// a method) — what linking an interface crate contributes to an artifact
/// that implements none of its traits. Everything shakes out.
fn unanchored_iface_module(with_function_export: bool) -> Vec<u8> {
    raw_fragment_module_bool(
        encode_fragments_unanchored(&[
            SpecFragment::SpecInterface {
                interface_fragment: SpecInterfaceFragment {
                    interface_name: s("Linked"),
                    interface_version: s("1.0.0"),
                    throws: vec![].try_into().unwrap(),
                    emits: vec![].try_into().unwrap(),
                },
            },
            SpecFragment::SpecMethod {
                method_fragment: SpecMethodFragment {
                    interface_name: s("Linked"),
                    interface_version: s("1.0.0"),
                    method: ContractSpecMethod {
                        name: s("ping"),
                        args: vec![].try_into().unwrap(),
                        return_type: ContractType::Void,
                    },
                },
            },
        ]),
        with_function_export,
    )
}

fn raw_fragment_module_bool(
    raw_blob: Vec<u8>,
    with_function_export: bool,
) -> Vec<u8> {
    raw_fragment_module(raw_blob, with_function_export.then_some("0"))
}

/// Minimal module carrying `raw_blob` as its raw spec section, optionally
/// exporting one no-op function under `export_fn_name`.
fn raw_fragment_module(
    raw_blob: Vec<u8>,
    export_fn_name: Option<&str>,
) -> Vec<u8> {
    let with_function_export = export_fn_name.is_some();
    let mut module = Module::new();
    if with_function_export {
        let mut types = TypeSection::new();
        types.ty().function([], []);
        module.section(&types);
        let mut functions = FunctionSection::new();
        functions.function(0);
        module.section(&functions);
    }
    let mut memory = MemorySection::new();
    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memory);
    if let Some(export_fn_name) = export_fn_name {
        let mut exports = ExportSection::new();
        exports.export(export_fn_name, ExportKind::Func, 0);
        module.section(&exports);
        let mut code = CodeSection::new();
        let mut body = Function::new([]);
        body.instruction(&Instruction::End);
        code.function(&body);
        module.section(&code);
    }
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw_blob.as_slice().into(),
    });
    module.finish()
}

#[test]
fn rewrite_strips_typedef_only_raw_sections_from_raw_contracts() {
    // A raw contract (hand-written export_name entrypoints, no #[contract])
    // that links SDK typedefs: the fragments are stripped and no spec is
    // emitted.
    let rewritten = typedef_only_module(true);
    let rewritten = rewrite_wasm(&rewritten).unwrap();

    assert!(custom_sections(&rewritten).is_empty());
}

#[test]
fn rewrite_rejects_typedef_only_modules_with_no_entrypoints() {
    // Typedefs, no interface, and no exported functions is what a contract
    // missing its #[contract] macro looks like (this caught exactly that
    // bug in genesis_deployer) — an uninvokable contract must not pass the
    // build.
    let err = rewrite_wasm(&typedef_only_module(false)).unwrap_err();

    assert!(matches!(err, SpecToolError::InvalidRawFragment(_)));
    assert!(err.to_string().contains("#[contract]"));
}

#[test]
fn rewrite_strips_all_shaken_raw_sections_from_raw_contracts() {
    // A raw contract (hand-written export_name entrypoints) that links an
    // interface crate: the crate's unanchored candidates all shake out, so
    // the artifact shape must match the typedef-only raw-contract case —
    // fragments stripped, NO spec section emitted. It must not flip to a
    // final section claiming `interfaces: []` just because a dependency
    // carried interface candidates.
    let rewritten = rewrite_wasm(&unanchored_iface_module(true)).unwrap();

    assert!(
        custom_sections(&rewritten).is_empty(),
        "expected no spec sections, got {:?}",
        custom_sections(&rewritten)
            .iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>()
    );
}

/// Export-section name of the entrypoint for `iface@vers::method`, via the
/// same contract-util function the shaker uses.
fn entrypoint_export(iface: &str, vers: &str, method: &str) -> String {
    groundhog_contract_util::entrypoint_export_name_for(
        Some(iface),
        vers,
        &groundhog_contract_util::NormalizedMethodSignature::new(method),
    )
}

#[test]
fn rewrite_shakes_phantom_methods_without_entrypoints() {
    // Colliding-key phantom: the anchored `Ledger` bucket carries `ping`
    // (real, exported) and `steal` (from a colliding candidate, no
    // entrypoint). The rewritten spec must advertise only `ping`.
    let raw = encode_fragments(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Ledger"),
                interface_version: s("1.0.0"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Ledger"),
                interface_version: s("1.0.0"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Ledger"),
                interface_version: s("1.0.0"),
                method: ContractSpecMethod {
                    name: s("steal"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ]);
    let module = raw_fragment_module(
        raw,
        Some(&entrypoint_export("Ledger", "1.0.0", "ping")),
    );
    let rewritten = rewrite_wasm(&module).unwrap();
    let spec = final_spec(&rewritten).unwrap();
    let methods: Vec<&str> = spec.interfaces[0]
        .methods
        .iter()
        .map(|m| spec_str(&m.name))
        .collect();
    assert_eq!(methods, vec!["ping"]);
}

#[test]
fn rewrite_keeps_carrier_interface_methods_despite_no_entrypoints() {
    // A spec-carrier artifact (export_interface_spec!) anchors an interface
    // and exports no functions AT ALL — the module-wide carrier exemption
    // must keep the full method list. (The missing-#[contract] guard does
    // not fire because the anchored interface keeps the spec non-empty.)
    let raw = encode_fragments(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Ledger"),
                interface_version: s("1.0.0"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Ledger"),
                interface_version: s("1.0.0"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ]);
    let module = raw_fragment_module(raw, None);
    let rewritten = rewrite_wasm(&module).unwrap();
    let spec = final_spec(&rewritten).unwrap();
    assert_eq!(spec.interfaces[0].methods.len(), 1);
}

#[test]
fn rewrite_rejects_all_shaken_modules_with_no_entrypoints() {
    // The missing-#[contract] guard must fire even when the dependency
    // graph contributed interface candidates: a module that anchors nothing
    // and exports no functions is an uninvokable contract regardless of
    // what its deps put in the raw section.
    let err = rewrite_wasm(&unanchored_iface_module(false)).unwrap_err();

    assert!(matches!(err, SpecToolError::InvalidRawFragment(_)));
    assert!(err.to_string().contains("#[contract]"));
}

// ---------------------------------------------------------------------------
// Regression: event shaking through the rewrite pipeline (the original bug)
//
// Before event shaking: an interface crate with multiple `#[contract]` traits
// both carrying `emits(...)` would cause events from ALL traits (including
// unimplemented ones) to appear in the final spec, because nothing removed
// event fragments that were only referenced by an unanchored interface.
//
// After the fix: only events reachable via surviving (anchored) interfaces
// survive `rewrite_wasm`.
// ---------------------------------------------------------------------------

/// Construct a minimal WASM module whose raw-spec section contains two
/// interface fragments (A and B) each referencing one event, plus the event
/// defs, but only an anchor for A.  This mirrors a contract that implements
/// trait A from an interface crate that also defines trait B.
fn event_shaking_module() -> Vec<u8> {
    fn iface(name: &str, emits_name: &str) -> SpecFragment {
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: ContractSpecString::try_from(name).unwrap(),
                interface_version: ContractSpecString::try_from("1.0.0")
                    .unwrap(),
                throws: vec![].try_into().unwrap(),
                emits: vec![ContractSpecString::try_from(emits_name).unwrap()]
                    .try_into()
                    .unwrap(),
            },
        }
    }

    fn ev_fragment(name: &str) -> SpecFragment {
        SpecFragment::SpecEvent {
            event_def: ContractEventDef {
                name: ContractSpecString::try_from(name).unwrap(),
                signature: [0u8; 32].into(),
                topics: vec![].try_into().unwrap(),
                fields: vec![].try_into().unwrap(),
            },
        }
    }

    fn method_fragment(iface_name: &str, method_name: &str) -> SpecFragment {
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: ContractSpecString::try_from(iface_name)
                    .unwrap(),
                interface_version: ContractSpecString::try_from("1.0.0")
                    .unwrap(),
                method: ContractSpecMethod {
                    name: ContractSpecString::try_from(method_name).unwrap(),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        }
    }

    fn anchor_fragment(iface_name: &str) -> SpecFragment {
        SpecFragment::InterfaceAnchor {
            interface_anchor: SpecInterfaceAnchor {
                interface_name: ContractSpecString::try_from(iface_name)
                    .unwrap(),
                interface_version: ContractSpecString::try_from("1.0.0")
                    .unwrap(),
            },
        }
    }

    // Manually encode fragments — do NOT use encode_fragments() here because
    // that helper auto-anchors every interface it sees.  We want only A
    // anchored so that B's event is shaken out.
    let fragments = [
        iface("ImplA", "AEvent"),
        method_fragment("ImplA", "a_method"),
        ev_fragment("AEvent"),
        iface("UnimplB", "BEvent"),
        method_fragment("UnimplB", "b_method"),
        ev_fragment("BEvent"),
        anchor_fragment("ImplA"), // only A is implemented
    ];

    let mut raw = Vec::new();
    for frag in &fragments {
        raw.extend(xdrs::txs::encode(frag).unwrap());
    }

    let mut module = wasm_encoder::Module::new();
    module.section(&wasm_encoder::CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: raw.as_slice().into(),
    });
    module.finish()
}

#[test]
fn rewrite_shakes_events_of_unimplemented_interfaces() {
    let module = event_shaking_module();
    let rewritten = rewrite_wasm(&module).unwrap();
    let spec = final_spec(&rewritten).expect("final spec must be present");

    // AEvent (from the implemented interface A) must survive.
    let event_names: Vec<&str> = spec
        .events
        .iter()
        .map(|e| std::str::from_utf8(&e.name).unwrap())
        .collect();
    assert!(
        event_names.contains(&"AEvent"),
        "AEvent from implemented interface A must be in the spec; got {event_names:?}"
    );

    // BEvent (from the unimplemented interface B) must be absent.
    assert!(
        !event_names.contains(&"BEvent"),
        "BEvent from unimplemented interface B must be shaken out; got {event_names:?}"
    );
}
