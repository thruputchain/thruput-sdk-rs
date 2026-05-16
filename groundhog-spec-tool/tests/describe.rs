use groundhog_contract_spec::{
    ContractSpec, ContractType, ContractTypeDef, ContractUnionCaseType,
};
use groundhog_spec_tool::describe::{build_spec, REGISTRY};
use std::collections::BTreeSet;

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

fn collect_named_refs(spec: &ContractSpec) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for type_def in spec.type_defs.iter() {
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

fn defined_names(spec: &ContractSpec) -> BTreeSet<String> {
    spec.type_defs
        .iter()
        .map(|td| {
            let name = match td {
                ContractTypeDef::TypeDefStruct { struct_def } => &struct_def.name,
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
        let spec = build_spec(entry)
            .unwrap_or_else(|e| panic!("build_spec failed for {}: {e:?}", entry.name));
        let refs = collect_named_refs(&spec);
        let defined = defined_names(&spec);
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
fn library_exn_json_snapshot() {
    let ty = groundhog_spec_tool::describe::lookup("LibraryExn")
        .expect("LibraryExn registered");
    let spec = build_spec(ty).expect("build_spec");
    let actual = groundhog_spec_tool::render_spec_json(&spec);

    // If this test fails, the spec output changed. If intentional, regenerate:
    //   cargo run -p groundhog-spec-tool -- describe LibraryExn --json
    let expected = include_str!("snapshots/library_exn.json").trim_end();
    assert_eq!(actual, expected, "JSON snapshot drift");
}
