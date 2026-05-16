use groundhog_contract_spec::{
    ContractType, ContractTypeDef, ContractUnionCaseType,
};
use groundhog_spec_tool::describe::{
    DescribeOutput, REGISTRY, build_describe,
};
use std::{collections::BTreeSet, process::Command};
use tempfile::tempdir;

fn walk_type(ty: &ContractType, refs: &mut BTreeSet<String>) {
    match ty {
        ContractType::Option { option_inner } => walk_type(option_inner, refs),
        ContractType::Vector { vector } => walk_type(&vector.element, refs),
        ContractType::Array { array } => walk_type(&array.element, refs),
        ContractType::Tuple { tuple } => {
            for item in tuple.iter() {
                walk_type(item, refs);
            }
        }
        ContractType::Named { named } => {
            refs.insert(std::str::from_utf8(named).unwrap().to_string());
        }
        _ => {}
    }
}

fn collect_named_refs(output: &DescribeOutput) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    walk_type(&output.exception.ty, &mut refs);
    for type_def in &output.type_defs {
        match type_def {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                for field in struct_def.fields.iter() {
                    walk_type(&field.ty, &mut refs);
                }
            }
            ContractTypeDef::TypeDefEnum { .. } => {}
            ContractTypeDef::TypeDefUnion { union_def } => {
                for case in union_def.cases.iter() {
                    match &case.case_type {
                        ContractUnionCaseType::UnionCaseTuple { tuple } => {
                            for ty in tuple.iter() {
                                walk_type(ty, &mut refs);
                            }
                        }
                        ContractUnionCaseType::UnionCaseStruct {
                            struct_fields,
                        } => {
                            for field in struct_fields.iter() {
                                walk_type(&field.ty, &mut refs);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    refs
}

fn defined_names(output: &DescribeOutput) -> BTreeSet<String> {
    output
        .type_defs
        .iter()
        .map(|td| {
            let name = match td {
                ContractTypeDef::TypeDefStruct { struct_def } => {
                    &struct_def.name
                }
                ContractTypeDef::TypeDefEnum { enum_def } => &enum_def.name,
                ContractTypeDef::TypeDefUnion { union_def } => &union_def.name,
                _ => panic!("unknown ContractTypeDef variant"),
            };
            std::str::from_utf8(name).unwrap().to_string()
        })
        .collect()
}

#[test]
fn every_registered_type_has_complete_closure() {
    for entry in REGISTRY {
        let output = build_describe(entry).unwrap_or_else(|e| {
            panic!("build_describe failed for {}: {e:?}", entry.name)
        });
        let refs = collect_named_refs(&output);
        let defined = defined_names(&output);
        let missing: Vec<_> = refs.difference(&defined).cloned().collect();
        assert!(
            missing.is_empty(),
            "type {}: dangling Named references: {:?}",
            entry.name,
            missing
        );
    }
}

#[test]
fn library_exn_has_nonzero_selector() {
    let output = build_describe(
        groundhog_spec_tool::describe::lookup("LibraryExn")
            .expect("LibraryExn registered"),
    )
    .expect("build_describe");
    assert_ne!(output.exception.selector, 0);
}

#[test]
fn library_exn_json_snapshot() {
    let ty = groundhog_spec_tool::describe::lookup("LibraryExn")
        .expect("LibraryExn registered");
    let output = build_describe(ty).expect("build_describe");
    let mut buf = Vec::new();
    groundhog_spec_tool::describe::write_output(
        &output,
        groundhog_spec_tool::describe::OutputFormat::Json,
        &mut buf,
    )
    .expect("write_output");
    let actual = String::from_utf8(buf).expect("valid utf8");
    let actual = actual.trim_end();

    let expected = include_str!("snapshots/library_exn.json").trim_end();
    assert_eq!(actual, expected, "JSON snapshot drift");
}

#[test]
fn describe_list_rejects_output_flags() {
    let tempdir = tempdir().unwrap();
    let out_path = tempdir.path().join("types.xdr");
    let cases = [
        vec!["describe", "--list", "--json"],
        vec!["describe", "--list", "--xdr"],
        vec!["describe", "--list", "-o", out_path.to_str().unwrap()],
    ];

    for args in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_groundhog-spec-tool"))
            .args(&args)
            .output()
            .unwrap();

        assert!(
            !output.status.success(),
            "expected {:?} to fail, stdout={:?}",
            args,
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--list"),
            "stderr for {:?}: {:?}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
