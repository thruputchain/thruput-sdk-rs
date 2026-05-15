use std::collections::BTreeMap;

pub mod raw;
pub mod rewrite;

pub use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractExceptionType,
    ContractField, ContractSpec, ContractSpecInterface, ContractSpecMethod,
    ContractSpecString, ContractStructDef, ContractType, ContractTypeDef,
    ContractUnionCase, ContractUnionCaseType, ContractUnionDef, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};
pub use raw::{
    FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME,
    parse_raw_fragments,
};

fn spec_str(s: &ContractSpecString) -> &str {
    std::str::from_utf8(s).expect("contract spec strings must be valid UTF-8")
}

/// Errors that can occur during spec canonicalization or rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecToolError {
    /// No spec fragments were found in the input.
    NoSpecFragments,
    /// A raw fragment failed validation (e.g. missing name or bad version).
    InvalidRawFragment(String),
    /// A method references an interface that has no interface fragment.
    MissingInterfaceFragment(String),
    /// A named type is referenced but no definition was provided.
    MissingTypeDef(String),
    /// Multiple conflicting definitions exist for the same type name.
    ConflictingTypeDef(String),
    /// Multiple interface fragments declare different `throws` for the same interface.
    ConflictingInterfaceThrows(String),
    /// Multiple conflicting method definitions share the same name in an interface.
    ConflictingMethod(String),
    /// An error occurred while parsing or writing the wasm binary.
    Wasm(String),
}

impl core::fmt::Display for SpecToolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoSpecFragments => write!(f, "no spec fragments"),
            Self::InvalidRawFragment(msg) => {
                write!(f, "invalid raw spec fragment: {msg}")
            }
            Self::MissingInterfaceFragment(msg) => {
                write!(f, "missing interface fragment for {msg}")
            }
            Self::MissingTypeDef(msg) => {
                write!(f, "missing contract type definition for {msg}")
            }
            Self::ConflictingTypeDef(msg) => {
                write!(f, "conflicting contract type definitions for {msg}")
            }
            Self::ConflictingInterfaceThrows(msg) => {
                write!(f, "conflicting throws metadata for interface {msg}")
            }
            Self::ConflictingMethod(msg) => {
                write!(f, "conflicting method definitions for {msg}")
            }
            Self::Wasm(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for SpecToolError {}

#[derive(Default)]
struct InterfaceBucket {
    throws: Option<Vec<ContractExceptionType>>,
    methods: Vec<ContractSpecMethod>,
}

/// Assembles raw spec fragments into a canonical [`ContractSpec`].
///
/// Deduplicates methods and type definitions, validates interface keys,
/// and resolves only the type definitions reachable from interface signatures.
pub fn canonicalize_spec(
    raw: &[SpecFragment],
) -> Result<ContractSpec, SpecToolError> {
    if raw.is_empty() {
        return Err(SpecToolError::NoSpecFragments);
    }

    let mut interfaces = BTreeMap::<
        (ContractSpecString, ContractSpecString),
        InterfaceBucket,
    >::new();
    let mut type_defs =
        BTreeMap::<ContractSpecString, Vec<ContractTypeDef>>::new();
    let mut exported_roots =
        std::collections::BTreeSet::<ContractSpecString>::new();

    for fragment in raw {
        match fragment {
            SpecFragment::SpecInterface {
                interface_fragment: fragment,
            } => {
                validate_interface_key(
                    spec_str(&fragment.interface_name),
                    spec_str(&fragment.interface_version),
                )?;

                let key = (
                    fragment.interface_name.clone(),
                    fragment.interface_version.clone(),
                );
                let throws = canonicalize_exception_list(&fragment.throws);
                let bucket = interfaces.entry(key.clone()).or_default();

                match &bucket.throws {
                    Some(existing) if *existing != throws => {
                        return Err(
                            SpecToolError::ConflictingInterfaceThrows(
                                interface_key_string(
                                    spec_str(&key.0),
                                    spec_str(&key.1),
                                ),
                            ),
                        );
                    }
                    Some(_) => {}
                    None => bucket.throws = Some(throws),
                }
            }
            SpecFragment::SpecMethod {
                method_fragment: fragment,
            } => {
                validate_interface_key(
                    spec_str(&fragment.interface_name),
                    spec_str(&fragment.interface_version),
                )?;
                validate_method(&fragment.method)?;

                let key = (
                    fragment.interface_name.clone(),
                    fragment.interface_version.clone(),
                );
                interfaces
                    .entry(key)
                    .or_default()
                    .methods
                    .push(fragment.method.clone());
            }
            SpecFragment::SpecTypeDef { type_def } => {
                let symbol = type_def_symbol(type_def).clone();
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
            SpecFragment::ExportedSpecTypeDef { type_def } => {
                let symbol = type_def_symbol(type_def).clone();
                exported_roots.insert(symbol.clone());
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
            _ => {}
        }
    }

    if interfaces.is_empty() {
        return Err(SpecToolError::NoSpecFragments);
    }

    for (symbol, defs) in &type_defs {
        if defs.len() > 1 {
            return Err(SpecToolError::ConflictingTypeDef(
                spec_str(symbol).to_owned(),
            ));
        }
    }

    let interfaces = interfaces
        .into_iter()
        .map(|((name, vers), mut bucket)| {
            bucket.methods.sort_by(|left, right| {
                left.name.cmp(&right.name).then_with(|| {
                    canonical_method_key(left)
                        .cmp(&canonical_method_key(right))
                })
            });
            let mut deduped_methods: Vec<ContractSpecMethod> =
                Vec::with_capacity(bucket.methods.len());
            for method in bucket.methods {
                if let Some(existing) = deduped_methods.last()
                    && existing.name == method.name
                {
                    if existing == &method {
                        continue;
                    }
                    return Err(SpecToolError::ConflictingMethod(format!(
                        "{}@{}::{}",
                        spec_str(&name),
                        spec_str(&vers),
                        spec_str(&method.name),
                    )));
                }
                deduped_methods.push(method);
            }

            let throws = bucket.throws.ok_or_else(|| {
                SpecToolError::MissingInterfaceFragment(interface_key_string(
                    spec_str(&name),
                    spec_str(&vers),
                ))
            })?;

            Ok(ContractSpecInterface {
                name,
                vers,
                throws: throws.try_into().unwrap(),
                methods: deduped_methods.try_into().unwrap(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let type_defs =
        resolve_reachable_type_defs(&interfaces, &type_defs, &exported_roots)?;
    Ok(ContractSpec {
        interfaces: interfaces.try_into().unwrap(),
        type_defs: type_defs.try_into().unwrap(),
    })
}

/// Assembles a [`ContractSpec`] whose `interfaces` list is empty and whose
/// `type_defs` are the transitive closure of `roots` over the supplied
/// fragments. Use this when you want to emit specs for named types without
/// any associated interface (e.g. the `describe` subcommand).
pub fn canonicalize_type_defs(
    raw: &[SpecFragment],
    roots: &[ContractSpecString],
) -> Result<ContractSpec, SpecToolError> {
    let mut type_defs =
        BTreeMap::<ContractSpecString, Vec<ContractTypeDef>>::new();
    let mut exported_roots =
        std::collections::BTreeSet::<ContractSpecString>::new();

    for fragment in raw {
        match fragment {
            SpecFragment::SpecTypeDef { type_def } => {
                let symbol = type_def_symbol(type_def).clone();
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
            SpecFragment::ExportedSpecTypeDef { type_def } => {
                let symbol = type_def_symbol(type_def).clone();
                exported_roots.insert(symbol.clone());
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
            _ => {}
        }
    }

    for symbol in roots {
        exported_roots.insert(symbol.clone());
    }

    for (symbol, defs) in &type_defs {
        if defs.len() > 1 {
            return Err(SpecToolError::ConflictingTypeDef(
                spec_str(symbol).to_owned(),
            ));
        }
    }

    let interfaces: Vec<ContractSpecInterface> = Vec::new();
    let resolved =
        resolve_reachable_type_defs(&interfaces, &type_defs, &exported_roots)?;
    Ok(ContractSpec {
        interfaces: interfaces.try_into().unwrap(),
        type_defs: resolved.try_into().unwrap(),
    })
}

/// Serializes a [`ContractSpec`] to a JSON string using the serde-derived
/// representation.  The output is guaranteed to round-trip through
/// [`parse_spec_json`].
pub fn spec_to_json(spec: &ContractSpec) -> Result<String, serde_json::Error> {
    serde_json::to_string(spec)
}

/// Deserializes a [`ContractSpec`] from a JSON string produced by
/// [`spec_to_json`].
pub fn parse_spec_json(json: &str) -> Result<ContractSpec, serde_json::Error> {
    serde_json::from_str(json)
}

/// Renders a [`ContractSpec`] as a human-friendly JSON string for debugging
/// and inspection.  This format includes a `format` tag and is **not**
/// guaranteed to be parseable back into a [`ContractSpec`].
pub fn render_spec_json(spec: &ContractSpec) -> String {
    use serde_json::json;

    let value = json!({
        "format": FINAL_FORMAT_TAG,
        "interfaces": spec.interfaces.iter().map(interface_to_json).collect::<Vec<_>>(),
        "type_defs": spec.type_defs.iter().map(type_def_to_json).collect::<Vec<_>>(),
    });

    serde_json::to_string(&value)
        .expect("contract spec must serialize to JSON")
}

fn validate_interface_key(
    interface_name: &str,
    interface_version: &str,
) -> Result<(), SpecToolError> {
    if interface_name.is_empty() || interface_version.is_empty() {
        return Err(SpecToolError::InvalidRawFragment(
            "interface fragments must include name and version".to_string(),
        ));
    }
    if !is_semver_triplet(interface_version) {
        return Err(SpecToolError::InvalidRawFragment(format!(
            "interface version must be MAJOR.MINOR.PATCH, got {interface_version}"
        )));
    }
    Ok(())
}

fn is_semver_triplet(version: &str) -> bool {
    let mut parts = version.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(major), Some(minor), Some(patch), None)
            if !major.is_empty()
                && !minor.is_empty()
                && !patch.is_empty()
                && major.chars().all(|ch| ch.is_ascii_digit())
                && minor.chars().all(|ch| ch.is_ascii_digit())
                && patch.chars().all(|ch| ch.is_ascii_digit())
    )
}

fn validate_method(method: &ContractSpecMethod) -> Result<(), SpecToolError> {
    if method.name.is_empty() {
        return Err(SpecToolError::InvalidRawFragment(
            "method fragments must include a method name".to_string(),
        ));
    }
    Ok(())
}

fn canonicalize_exception_list(
    types: &[ContractExceptionType],
) -> Vec<ContractExceptionType> {
    let mut types = types.to_vec();
    types.sort_by(|left, right| {
        canonical_type_key(&left.ty)
            .cmp(&canonical_type_key(&right.ty))
            .then_with(|| left.selector.cmp(&right.selector))
    });
    types.dedup();
    types
}

fn resolve_reachable_type_defs(
    interfaces: &[ContractSpecInterface],
    defs: &BTreeMap<ContractSpecString, Vec<ContractTypeDef>>,
    exported_roots: &std::collections::BTreeSet<ContractSpecString>,
) -> Result<Vec<ContractTypeDef>, SpecToolError> {
    let mut resolved = BTreeMap::<ContractSpecString, ContractTypeDef>::new();
    let mut pending = Vec::<ContractSpecString>::new();

    for interface in interfaces {
        for exn in interface.throws.iter() {
            collect_named_types(&exn.ty, defs, &mut resolved, &mut pending)?;
        }
        for method in interface.methods.iter() {
            for arg in method.args.iter() {
                collect_named_types(
                    &arg.ty,
                    defs,
                    &mut resolved,
                    &mut pending,
                )?;
            }
            collect_named_types(
                &method.return_type,
                defs,
                &mut resolved,
                &mut pending,
            )?;
        }
    }

    for symbol in exported_roots {
        collect_named_types(
            &ContractType::Named {
                named: symbol.clone(),
            },
            defs,
            &mut resolved,
            &mut pending,
        )?;
    }

    while let Some(symbol) = pending.pop() {
        match resolved.get(&symbol).cloned() {
            Some(ContractTypeDef::TypeDefStruct { struct_def }) => {
                for field in struct_def.fields.iter() {
                    collect_named_types(
                        &field.ty,
                        defs,
                        &mut resolved,
                        &mut pending,
                    )?;
                }
            }
            Some(ContractTypeDef::TypeDefEnum { .. }) => {}
            Some(ContractTypeDef::TypeDefUnion { union_def }) => {
                for case in union_def.cases.iter() {
                    collect_union_case_named_types(
                        &case.case_type,
                        defs,
                        &mut resolved,
                        &mut pending,
                    )?;
                }
            }
            Some(_) => panic!("unknown ContractTypeDef variant"),
            None => {
                return Err(SpecToolError::MissingTypeDef(
                    spec_str(&symbol).to_owned(),
                ));
            }
        }
    }

    Ok(resolved.into_values().collect())
}

fn collect_named_types(
    ty: &ContractType,
    defs: &BTreeMap<ContractSpecString, Vec<ContractTypeDef>>,
    resolved: &mut BTreeMap<ContractSpecString, ContractTypeDef>,
    pending: &mut Vec<ContractSpecString>,
) -> Result<(), SpecToolError> {
    match ty {
        ContractType::Option { option_inner } => {
            collect_named_types(option_inner, defs, resolved, pending)?;
        }
        ContractType::Vector { vector } => {
            let element = &vector.element;
            collect_named_types(element, defs, resolved, pending)?;
        }
        ContractType::Array { array } => {
            let element = &array.element;
            collect_named_types(element, defs, resolved, pending)?;
        }
        ContractType::Tuple { tuple } => {
            for item in tuple.iter() {
                collect_named_types(item, defs, resolved, pending)?;
            }
        }
        ContractType::Named { named } => {
            let Some(candidates) = defs.get(named) else {
                return Err(SpecToolError::MissingTypeDef(
                    spec_str(named).to_owned(),
                ));
            };
            if candidates.len() > 1 {
                return Err(SpecToolError::ConflictingTypeDef(
                    spec_str(named).to_owned(),
                ));
            }
            if !resolved.contains_key(named) {
                resolved.insert(named.clone(), candidates[0].clone());
                pending.push(named.clone());
            }
        }
        ContractType::Void
        | ContractType::Bool
        | ContractType::U8
        | ContractType::U32
        | ContractType::I32
        | ContractType::U64
        | ContractType::I64
        | ContractType::U128
        | ContractType::I128
        | ContractType::String
        | ContractType::Bytes
        | ContractType::Address => {}
        _ => {}
    }

    Ok(())
}

fn collect_union_case_named_types(
    case_type: &ContractUnionCaseType,
    defs: &BTreeMap<ContractSpecString, Vec<ContractTypeDef>>,
    resolved: &mut BTreeMap<ContractSpecString, ContractTypeDef>,
    pending: &mut Vec<ContractSpecString>,
) -> Result<(), SpecToolError> {
    match case_type {
        ContractUnionCaseType::UnionCaseVoid => {}
        ContractUnionCaseType::UnionCaseTuple { tuple } => {
            for ty in tuple.iter() {
                collect_named_types(ty, defs, resolved, pending)?;
            }
        }
        ContractUnionCaseType::UnionCaseStruct { struct_fields } => {
            for field in struct_fields.iter() {
                collect_named_types(&field.ty, defs, resolved, pending)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn canonical_method_key(method: &ContractSpecMethod) -> String {
    let mut key = spec_str(&method.name).to_owned();
    key.push('|');
    for arg in method.args.iter() {
        key.push_str(spec_str(&arg.name));
        key.push(':');
        key.push_str(&canonical_type_key(&arg.ty));
        key.push('|');
    }
    key.push_str("->");
    key.push_str(&canonical_type_key(&method.return_type));
    key
}

fn canonical_type_key(ty: &ContractType) -> String {
    match ty {
        ContractType::Void => "void".to_string(),
        ContractType::Bool => "bool".to_string(),
        ContractType::U8 => "u8".to_string(),
        ContractType::U32 => "u32".to_string(),
        ContractType::I32 => "i32".to_string(),
        ContractType::U64 => "u64".to_string(),
        ContractType::I64 => "i64".to_string(),
        ContractType::U128 => "u128".to_string(),
        ContractType::I128 => "i128".to_string(),
        ContractType::String => "string".to_string(),
        ContractType::Bytes => "bytes".to_string(),
        ContractType::Address => "address".to_string(),
        ContractType::Option { option_inner } => {
            format!("option({})", canonical_type_key(option_inner))
        }
        ContractType::Vector { vector } => {
            let element = &vector.element;
            format!(
                "vector({}, {})",
                canonical_type_key(element),
                vector.max_len
            )
        }
        ContractType::Tuple { tuple } => {
            let mut key = String::from("tuple(");
            for (index, item) in tuple.iter().enumerate() {
                if index > 0 {
                    key.push(',');
                }
                key.push_str(&canonical_type_key(item));
            }
            key.push(')');
            key
        }
        ContractType::Array { array } => {
            let element = &array.element;
            format!("array({}, {})", canonical_type_key(element), array.len)
        }
        ContractType::Named { named } => {
            format!("named({})", spec_str(named))
        }
        _ => panic!("unknown ContractType variant"),
    }
}

fn type_def_symbol(type_def: &ContractTypeDef) -> &ContractSpecString {
    match type_def {
        ContractTypeDef::TypeDefStruct { struct_def } => &struct_def.name,
        ContractTypeDef::TypeDefEnum { enum_def } => &enum_def.name,
        ContractTypeDef::TypeDefUnion { union_def } => &union_def.name,
        _ => panic!("unknown ContractTypeDef variant"),
    }
}

fn interface_key_string(name: &str, version: &str) -> String {
    format!("{name}@{version}")
}

fn interface_to_json(interface: &ContractSpecInterface) -> serde_json::Value {
    serde_json::json!({
        "name": spec_str(&interface.name),
        "version": spec_str(&interface.vers),
        "throws": interface.throws.iter().map(exception_type_to_json).collect::<Vec<_>>(),
        "methods": interface.methods.iter().map(method_to_json).collect::<Vec<_>>(),
    })
}

fn exception_type_to_json(exn: &ContractExceptionType) -> serde_json::Value {
    serde_json::json!({
        "selector": format!("0x{:016x}", exn.selector),
        "type": type_to_json(&exn.ty),
    })
}

fn method_to_json(method: &ContractSpecMethod) -> serde_json::Value {
    serde_json::json!({
        "name": spec_str(&method.name),
        "args": method.args.iter().map(field_to_json).collect::<Vec<_>>(),
        "return_type": type_to_json(&method.return_type),
    })
}

fn field_to_json(field: &ContractField) -> serde_json::Value {
    serde_json::json!({
        "name": spec_str(&field.name),
        "ty": type_to_json(&field.ty),
    })
}

fn type_to_json(ty: &ContractType) -> serde_json::Value {
    use serde_json::json;
    match ty {
        ContractType::Void => json!({"kind": "void"}),
        ContractType::Bool => json!({"kind": "bool"}),
        ContractType::U8 => json!({"kind": "u8"}),
        ContractType::U32 => json!({"kind": "u32"}),
        ContractType::I32 => json!({"kind": "i32"}),
        ContractType::U64 => json!({"kind": "u64"}),
        ContractType::I64 => json!({"kind": "i64"}),
        ContractType::U128 => json!({"kind": "u128"}),
        ContractType::I128 => json!({"kind": "i128"}),
        ContractType::String => json!({"kind": "string"}),
        ContractType::Bytes => json!({"kind": "bytes"}),
        ContractType::Address => json!({"kind": "address"}),
        ContractType::Option { option_inner } => {
            json!({"kind": "option", "inner": type_to_json(option_inner)})
        }
        ContractType::Vector { vector } => {
            let element = &vector.element;
            json!({"kind": "vec", "inner": type_to_json(element), "max_len": vector.max_len})
        }
        ContractType::Tuple { tuple } => {
            json!({"kind": "tuple", "items": tuple.iter().map(type_to_json).collect::<Vec<_>>()})
        }
        ContractType::Named { named } => {
            json!({"kind": "named", "name": spec_str(named)})
        }
        ContractType::Array { array } => {
            let element = &array.element;
            json!({"kind": "array", "len": array.len, "element": type_to_json(element)})
        }
        _ => panic!("unknown ContractType variant"),
    }
}

fn type_def_to_json(type_def: &ContractTypeDef) -> serde_json::Value {
    use serde_json::json;
    match type_def {
        ContractTypeDef::TypeDefStruct { struct_def } => json!({
            "kind": "struct",
            "name": spec_str(&struct_def.name),
            "fields": struct_def.fields.iter().map(field_to_json).collect::<Vec<_>>(),
        }),
        ContractTypeDef::TypeDefEnum { enum_def } => json!({
            "kind": "enum",
            "name": spec_str(&enum_def.name),
            "variants": enum_def.variants.iter().map(|v| json!({"name": spec_str(&v.name), "value": v.value})).collect::<Vec<_>>(),
        }),
        ContractTypeDef::TypeDefUnion { union_def } => json!({
            "kind": "union",
            "name": spec_str(&union_def.name),
            "cases": union_def.cases.iter().map(|c| json!({
                "name": spec_str(&c.name),
                "discriminant": c.discriminant,
                "case_type": union_case_type_to_json(&c.case_type),
            })).collect::<Vec<_>>(),
        }),
        _ => panic!("unknown ContractTypeDef variant"),
    }
}

fn union_case_type_to_json(
    case_type: &ContractUnionCaseType,
) -> serde_json::Value {
    use serde_json::json;
    match case_type {
        ContractUnionCaseType::UnionCaseVoid => json!({"kind": "void"}),
        ContractUnionCaseType::UnionCaseTuple { tuple } => json!({
            "kind": "tuple",
            "items": tuple.iter().map(type_to_json).collect::<Vec<_>>(),
        }),
        ContractUnionCaseType::UnionCaseStruct { struct_fields } => json!({
            "kind": "struct",
            "fields": struct_fields.iter().map(field_to_json).collect::<Vec<_>>(),
        }),
        _ => panic!("unknown ContractUnionCaseType variant"),
    }
}

#[cfg(test)]
mod canonicalize_type_defs_tests {
    use super::*;
    use groundhog_contract_spec::{
        ContractEnumDef, ContractEnumVariant, ContractTypeDef,
    };

    fn enum_def(name: &str) -> ContractTypeDef {
        ContractTypeDef::TypeDefEnum {
            enum_def: ContractEnumDef {
                name: name.as_bytes().try_into().unwrap(),
                variants: vec![ContractEnumVariant {
                    name: b"A".as_slice().try_into().unwrap(),
                    value: 0,
                }]
                .try_into()
                .unwrap(),
            },
        }
    }

    #[test]
    fn canonicalize_type_defs_resolves_root() {
        let fragments = vec![SpecFragment::SpecTypeDef {
            type_def: enum_def("MyEnum"),
        }];
        let roots = vec![b"MyEnum".as_slice().try_into().unwrap()];

        let spec = canonicalize_type_defs(&fragments, &roots)
            .expect("canonicalize_type_defs should succeed");

        assert!(spec.interfaces.is_empty());
        assert_eq!(spec.type_defs.len(), 1);
    }

    #[test]
    fn canonicalize_type_defs_errors_on_missing_root() {
        let fragments: Vec<SpecFragment> = vec![];
        let roots = vec![b"MissingType".as_slice().try_into().unwrap()];

        let err = canonicalize_type_defs(&fragments, &roots)
            .expect_err("missing root should error");
        assert!(matches!(err, SpecToolError::MissingTypeDef(_)));
    }
}
