use std::collections::BTreeMap;

pub mod model;
pub mod raw;
pub mod rewrite;

pub use model::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpec,
    ContractSpecInterface, ContractSpecMethod, ContractStructDef,
    ContractType, ContractTypeDef, ContractUnionCase, ContractUnionDef,
    InterfaceId, RawInterfaceFragment, SpecFragment, SpecInterfaceFragment,
    SpecMethodFragment,
};
pub use raw::{
    FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_RECORD_TAG, RAW_SECTION_NAME,
    encode_raw_fragment, parse_raw_fragment_line, parse_raw_fragments,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecToolError {
    NoSpecFragments,
    InvalidRawFragment(String),
    MissingInterfaceFragment(String),
    MissingTypeDef(String),
    ConflictingTypeDef(String),
    ConflictingInterfaceThrows(String),
    Wasm(String),
}

impl core::fmt::Display for SpecToolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoSpecFragments => write!(f, "no spec fragments"),
            Self::InvalidRawFragment(msg)
            | Self::Wasm(msg)
            | Self::MissingInterfaceFragment(msg)
            | Self::MissingTypeDef(msg)
            | Self::ConflictingTypeDef(msg)
            | Self::ConflictingInterfaceThrows(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for SpecToolError {}

#[derive(Default)]
struct InterfaceBucket {
    throws: Option<Vec<ContractType>>,
    methods: Vec<ContractSpecMethod>,
}

pub fn canonicalize_spec(
    package_name: &str,
    raw: &[SpecFragment],
) -> Result<ContractSpec, SpecToolError> {
    if raw.is_empty() {
        return Err(SpecToolError::NoSpecFragments);
    }

    let mut interfaces = BTreeMap::<(String, String), InterfaceBucket>::new();
    let mut type_defs = BTreeMap::<String, Vec<ContractTypeDef>>::new();

    for fragment in raw {
        match fragment {
            SpecFragment::Interface(fragment)
                if fragment.package_name == package_name =>
            {
                validate_interface_key(
                    &fragment.interface_name,
                    &fragment.interface_version,
                )?;

                let key = (
                    fragment.interface_name.clone(),
                    fragment.interface_version.clone(),
                );
                let throws = canonicalize_type_list(&fragment.throws);
                let bucket = interfaces.entry(key.clone()).or_default();

                match &bucket.throws {
                    Some(existing) if *existing != throws => {
                        return Err(
                            SpecToolError::ConflictingInterfaceThrows(
                                interface_key_string(&key.0, &key.1),
                            ),
                        );
                    }
                    Some(_) => {}
                    None => bucket.throws = Some(throws),
                }
            }
            SpecFragment::Method(fragment)
                if fragment.package_name == package_name =>
            {
                validate_interface_key(
                    &fragment.interface_name,
                    &fragment.interface_version,
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
            SpecFragment::TypeDef(type_def) => {
                let symbol = type_def_symbol(type_def).to_owned();
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

    let interfaces = interfaces
        .into_iter()
        .map(|((name, version), mut bucket)| {
            bucket.methods.sort_by(|left, right| {
                canonical_method_key(left).cmp(&canonical_method_key(right))
            });
            bucket.methods.dedup();

            let throws = bucket.throws.ok_or_else(|| {
                SpecToolError::MissingInterfaceFragment(interface_key_string(
                    &name, &version,
                ))
            })?;

            Ok(ContractSpecInterface {
                name,
                version,
                throws,
                methods: bucket.methods,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let type_defs = resolve_reachable_type_defs(&interfaces, &type_defs)?;
    Ok(ContractSpec {
        interfaces,
        type_defs,
    })
}

pub fn canonicalize_interfaces(
    raw: &[RawInterfaceFragment],
) -> Result<Vec<InterfaceId>, SpecToolError> {
    if raw.is_empty() {
        return Err(SpecToolError::NoSpecFragments);
    }

    let mut interfaces = Vec::with_capacity(raw.len());

    for fragment in raw {
        validate_interface_key(
            &fragment.interface_name,
            &fragment.interface_version,
        )?;
        interfaces.push(InterfaceId::new(
            fragment.interface_name.clone(),
            fragment.interface_version.clone(),
        ));
    }

    interfaces.sort();
    interfaces.dedup();
    Ok(interfaces)
}

pub fn render_spec_json<T>(value: &T) -> String
where
    T: RenderSpecJson + ?Sized,
{
    value.render_spec_json()
}

pub trait RenderSpecJson {
    fn render_spec_json(&self) -> String;
}

impl RenderSpecJson for ContractSpec {
    fn render_spec_json(&self) -> String {
        let mut json = String::from("{\"format\":\"");
        json.push_str(FINAL_FORMAT_TAG);
        json.push_str("\",\"interfaces\":[");

        for (index, interface) in self.interfaces.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            json.push_str("{\"name\":\"");
            push_json_string(&mut json, &interface.name);
            json.push_str("\",\"version\":\"");
            push_json_string(&mut json, &interface.version);
            json.push_str("\",\"throws\":[");
            for (throws_index, ty) in interface.throws.iter().enumerate() {
                if throws_index > 0 {
                    json.push(',');
                }
                push_type_json(&mut json, ty);
            }
            json.push_str("],\"methods\":[");
            for (method_index, method) in interface.methods.iter().enumerate()
            {
                if method_index > 0 {
                    json.push(',');
                }
                json.push_str("{\"name\":\"");
                push_json_string(&mut json, &method.name);
                json.push_str("\",\"args\":[");
                for (arg_index, arg) in method.args.iter().enumerate() {
                    if arg_index > 0 {
                        json.push(',');
                    }
                    json.push_str("{\"name\":\"");
                    push_json_string(&mut json, &arg.name);
                    json.push_str("\",\"ty\":");
                    push_type_json(&mut json, &arg.ty);
                    json.push('}');
                }
                json.push_str("],\"return_type\":");
                push_type_json(&mut json, &method.return_type);
                json.push('}');
            }
            json.push_str("]}");
        }

        json.push_str("],\"type_defs\":[");
        for (index, type_def) in self.type_defs.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            push_type_def_json(&mut json, type_def);
        }
        json.push_str("]}");
        json
    }
}

impl RenderSpecJson for Vec<InterfaceId> {
    fn render_spec_json(&self) -> String {
        self.as_slice().render_spec_json()
    }
}

impl RenderSpecJson for [InterfaceId] {
    fn render_spec_json(&self) -> String {
        let mut json = String::from("{\"format\":\"");
        json.push_str(FINAL_FORMAT_TAG);
        json.push_str("\",\"interfaces\":[");

        for (index, interface) in self.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            json.push_str("{\"interface_name\":\"");
            push_json_string(&mut json, &interface.interface_name);
            json.push_str("\",\"interface_version\":\"");
            push_json_string(&mut json, &interface.interface_version);
            json.push_str("\"}");
        }

        json.push_str("]}");
        json
    }
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
    Ok(())
}

fn validate_method(method: &ContractSpecMethod) -> Result<(), SpecToolError> {
    if method.name.is_empty() {
        return Err(SpecToolError::InvalidRawFragment(
            "method fragments must include a method name".to_string(),
        ));
    }
    Ok(())
}

fn canonicalize_type_list(types: &[ContractType]) -> Vec<ContractType> {
    let mut types = types.to_vec();
    types.sort_by(|left, right| {
        canonical_type_key(left).cmp(&canonical_type_key(right))
    });
    types.dedup();
    types
}

fn resolve_reachable_type_defs(
    interfaces: &[ContractSpecInterface],
    defs: &BTreeMap<String, Vec<ContractTypeDef>>,
) -> Result<Vec<ContractTypeDef>, SpecToolError> {
    let mut resolved = BTreeMap::<String, ContractTypeDef>::new();
    let mut pending = Vec::<String>::new();

    for interface in interfaces {
        for ty in &interface.throws {
            collect_named_types(ty, defs, &mut resolved, &mut pending)?;
        }
        for method in &interface.methods {
            for arg in &method.args {
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

    while let Some(symbol) = pending.pop() {
        match resolved.get(symbol.as_str()).cloned() {
            Some(ContractTypeDef::Struct(def)) => {
                for field in &def.fields {
                    collect_named_types(
                        &field.ty,
                        defs,
                        &mut resolved,
                        &mut pending,
                    )?;
                }
            }
            Some(ContractTypeDef::Enum(_)) => {}
            Some(ContractTypeDef::Union(def)) => {
                for case in &def.cases {
                    if let Some(case_type) = &case.case_type {
                        collect_named_types(
                            case_type,
                            defs,
                            &mut resolved,
                            &mut pending,
                        )?;
                    }
                }
            }
            None => return Err(SpecToolError::MissingTypeDef(symbol)),
        }
    }

    Ok(resolved.into_values().collect())
}

fn collect_named_types(
    ty: &ContractType,
    defs: &BTreeMap<String, Vec<ContractTypeDef>>,
    resolved: &mut BTreeMap<String, ContractTypeDef>,
    pending: &mut Vec<String>,
) -> Result<(), SpecToolError> {
    match ty {
        ContractType::Option(inner) | ContractType::Vec(inner) => {
            collect_named_types(inner, defs, resolved, pending)?;
        }
        ContractType::Tuple(items) => {
            for item in items {
                collect_named_types(item, defs, resolved, pending)?;
            }
        }
        ContractType::Named(name) => {
            let Some(candidates) = defs.get(name.as_str()) else {
                return Err(SpecToolError::MissingTypeDef(name.clone()));
            };
            if candidates.len() > 1 {
                return Err(SpecToolError::ConflictingTypeDef(name.clone()));
            }
            if !resolved.contains_key(name.as_str()) {
                resolved.insert(name.clone(), candidates[0].clone());
                pending.push(name.clone());
            }
        }
        ContractType::Void
        | ContractType::Bool
        | ContractType::U32
        | ContractType::I32
        | ContractType::U64
        | ContractType::I64
        | ContractType::U128
        | ContractType::I128
        | ContractType::String
        | ContractType::Bytes
        | ContractType::Address => {}
    }

    Ok(())
}

fn canonical_method_key(method: &ContractSpecMethod) -> String {
    let mut key = method.name.clone();
    key.push('|');
    for arg in &method.args {
        key.push_str(&arg.name);
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
        ContractType::U32 => "u32".to_string(),
        ContractType::I32 => "i32".to_string(),
        ContractType::U64 => "u64".to_string(),
        ContractType::I64 => "i64".to_string(),
        ContractType::U128 => "u128".to_string(),
        ContractType::I128 => "i128".to_string(),
        ContractType::String => "string".to_string(),
        ContractType::Bytes => "bytes".to_string(),
        ContractType::Address => "address".to_string(),
        ContractType::Option(inner) => {
            format!("option({})", canonical_type_key(inner))
        }
        ContractType::Vec(inner) => {
            format!("vec({})", canonical_type_key(inner))
        }
        ContractType::Tuple(items) => {
            let mut key = String::from("tuple(");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    key.push(',');
                }
                key.push_str(&canonical_type_key(item));
            }
            key.push(')');
            key
        }
        ContractType::Named(name) => format!("named({name})"),
    }
}

fn type_def_symbol(type_def: &ContractTypeDef) -> &str {
    match type_def {
        ContractTypeDef::Struct(def) => &def.name,
        ContractTypeDef::Enum(def) => &def.name,
        ContractTypeDef::Union(def) => &def.name,
    }
}

fn interface_key_string(name: &str, version: &str) -> String {
    format!("{name}@{version}")
}

fn push_type_json(out: &mut String, ty: &ContractType) {
    match ty {
        ContractType::Void => out.push_str("{\"kind\":\"void\"}"),
        ContractType::Bool => out.push_str("{\"kind\":\"bool\"}"),
        ContractType::U32 => out.push_str("{\"kind\":\"u32\"}"),
        ContractType::I32 => out.push_str("{\"kind\":\"i32\"}"),
        ContractType::U64 => out.push_str("{\"kind\":\"u64\"}"),
        ContractType::I64 => out.push_str("{\"kind\":\"i64\"}"),
        ContractType::U128 => out.push_str("{\"kind\":\"u128\"}"),
        ContractType::I128 => out.push_str("{\"kind\":\"i128\"}"),
        ContractType::String => out.push_str("{\"kind\":\"string\"}"),
        ContractType::Bytes => out.push_str("{\"kind\":\"bytes\"}"),
        ContractType::Address => out.push_str("{\"kind\":\"address\"}"),
        ContractType::Option(inner) => {
            out.push_str("{\"kind\":\"option\",\"inner\":");
            push_type_json(out, inner);
            out.push('}');
        }
        ContractType::Vec(inner) => {
            out.push_str("{\"kind\":\"vec\",\"inner\":");
            push_type_json(out, inner);
            out.push('}');
        }
        ContractType::Tuple(items) => {
            out.push_str("{\"kind\":\"tuple\",\"items\":[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_type_json(out, item);
            }
            out.push_str("]}");
        }
        ContractType::Named(name) => {
            out.push_str("{\"kind\":\"named\",\"name\":\"");
            push_json_string(out, name);
            out.push_str("\"}");
        }
    }
}

fn push_type_def_json(out: &mut String, type_def: &ContractTypeDef) {
    match type_def {
        ContractTypeDef::Struct(def) => {
            out.push_str("{\"kind\":\"struct\",\"name\":\"");
            push_json_string(out, &def.name);
            out.push_str("\",\"fields\":[");
            for (index, field) in def.fields.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str("{\"name\":\"");
                push_json_string(out, &field.name);
                out.push_str("\",\"ty\":");
                push_type_json(out, &field.ty);
                out.push('}');
            }
            out.push_str("]}");
        }
        ContractTypeDef::Enum(def) => {
            out.push_str("{\"kind\":\"enum\",\"name\":\"");
            push_json_string(out, &def.name);
            out.push_str("\",\"variants\":[");
            for (index, variant) in def.variants.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str("{\"name\":\"");
                push_json_string(out, &variant.name);
                out.push_str("\",\"value\":");
                out.push_str(&variant.value.to_string());
                out.push('}');
            }
            out.push_str("]}");
        }
        ContractTypeDef::Union(def) => {
            out.push_str("{\"kind\":\"union\",\"name\":\"");
            push_json_string(out, &def.name);
            out.push_str("\",\"cases\":[");
            for (index, case) in def.cases.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str("{\"name\":\"");
                push_json_string(out, &case.name);
                out.push_str("\",\"discriminant\":");
                out.push_str(&case.discriminant.to_string());
                out.push_str(",\"case_type\":");
                match &case.case_type {
                    Some(case_type) => push_type_json(out, case_type),
                    None => out.push_str("null"),
                }
                out.push('}');
            }
            out.push_str("]}");
        }
    }
}

fn push_json_string(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                use core::fmt::Write as _;
                write!(out, "\\u{:04x}", ch as u32).unwrap();
            }
            ch => out.push(ch),
        }
    }
}
