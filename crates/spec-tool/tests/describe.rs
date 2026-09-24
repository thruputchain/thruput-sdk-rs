use groundhog_contract_spec::{
    ContractSpecId, ContractType, ContractTypeDef, ContractUnionCaseType,
};
use groundhog_spec_tool::describe::{
    DescribeOutput, REGISTRY, build_describe,
};
use std::{collections::BTreeSet, process::Command};
use tempfile::tempdir;

fn walk_type(ty: &ContractType, refs: &mut BTreeSet<ContractSpecId>) {
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
            refs.insert(named.clone());
        }
        _ => {}
    }
}

fn collect_named_refs(output: &DescribeOutput) -> BTreeSet<ContractSpecId> {
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

fn defined_names(output: &DescribeOutput) -> BTreeSet<ContractSpecId> {
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
            name.clone()
        })
        .collect()
}

#[test]
fn every_registered_type_has_complete_closure() {
    for entry in REGISTRY {
        let output = build_describe(entry);
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
    );
    assert_ne!(output.exception.selector, 0);
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
