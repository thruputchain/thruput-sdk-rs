use std::collections::BTreeMap;

pub mod describe;
pub mod raw;
pub mod rewrite;

pub use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractEventDef,
    ContractExceptionType, ContractField, ContractSpec, ContractSpecInterface,
    ContractSpecMethod, ContractSpecString, ContractStructDef, ContractType,
    ContractTypeDef, ContractUnionCase, ContractUnionCaseType,
    ContractUnionDef, SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};
pub use raw::{
    FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME,
    parse_raw_fragments,
};

/// Decode a spec string, rejecting non-UTF-8 bytes.
///
/// `ContractSpecString` is raw bytes on the wire (XDR decode performs no
/// UTF-8 validation), so malformed wasm can carry arbitrary bytes here.
fn spec_str(s: &ContractSpecString) -> Result<&str, SpecToolError> {
    std::str::from_utf8(s).map_err(|_| {
        SpecToolError::InvalidRawFragment(format!(
            "spec string is not valid UTF-8: {:?}",
            String::from_utf8_lossy(s)
        ))
    })
}

/// Lossy spec-string decoding for error messages and debug rendering, where
/// failing on malformed bytes would hide the more useful diagnostic.
fn spec_str_lossy(s: &ContractSpecString) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(s)
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
    /// Two `throws` entries share a selector but have different payload
    /// types, making selector-based exception decoding ambiguous.
    ConflictingExceptionSelector(String),
    /// Multiple interface fragments declare different `throws` for the same interface.
    ConflictingInterfaceThrows(String),
    /// Multiple conflicting method definitions share the same name in an interface.
    ConflictingMethod(String),
    /// Multiple conflicting event definitions exist for the same event name.
    ConflictingEventDef(String),
    /// Multiple interface fragments declare different `emits` for the same interface.
    ConflictingInterfaceEmits(String),
    /// An interface's `emits` references an event with no definition fragment.
    MissingEventDef(String),
    /// The spec contains a variant this tool version does not understand
    /// (e.g. it was produced by a newer toolchain). Rejecting it loudly
    /// beats silently dropping or mangling parts of the spec.
    UnsupportedVariant(String),
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
            Self::ConflictingExceptionSelector(msg) => {
                write!(f, "conflicting exception selector: {msg}")
            }
            Self::ConflictingInterfaceThrows(msg) => {
                write!(f, "conflicting throws metadata for interface {msg}")
            }
            Self::ConflictingMethod(msg) => {
                write!(f, "conflicting method definitions for {msg}")
            }
            Self::ConflictingEventDef(msg) => {
                write!(f, "conflicting event definitions for {msg}")
            }
            Self::ConflictingInterfaceEmits(msg) => {
                write!(f, "conflicting emits metadata for interface {msg}")
            }
            Self::MissingEventDef(msg) => {
                write!(f, "missing event definition for {msg}")
            }
            Self::UnsupportedVariant(msg) => {
                write!(f, "unsupported spec variant: {msg}")
            }
            Self::Wasm(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for SpecToolError {}

#[derive(Default)]
struct InterfaceBucket {
    /// Distinct (throws, emits) candidates from this interface's fragments;
    /// emits pre-canonicalized (sorted, deduped), throws kept raw. Conflicts
    /// among candidates — including selector conflicts within a throws list —
    /// are only checked for anchored interfaces after shaking: unrelated
    /// interface crates can collide on a key (trait-ident name, crate-version
    /// version), and a caller linking both must not fail on their
    /// disagreement.
    candidates: Vec<(Vec<ContractExceptionType>, Vec<ContractSpecString>)>,
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
    // Interfaces an impl (or `export_interface_spec!` carrier) anchored for
    // inclusion. A trait's interface spec is a *candidate*: it lands in the
    // output only if anchored. Shaking is strict — an empty set drops every
    // interface (there is intentionally no "include all when unanchored"
    // fallback; a real contract always anchors its own interface).
    let mut anchored_interfaces = std::collections::BTreeSet::<(
        ContractSpecString,
        ContractSpecString,
    )>::new();
    let mut events =
        BTreeMap::<ContractSpecString, Vec<ContractEventDef>>::new();
    let mut exported_events =
        std::collections::BTreeSet::<ContractSpecString>::new();

    for fragment in raw {
        match fragment {
            SpecFragment::SpecInterface {
                interface_fragment: fragment,
            } => {
                validate_interface_key(
                    spec_str(&fragment.interface_name)?,
                    spec_str(&fragment.interface_version)?,
                )?;

                let key = (
                    fragment.interface_name.clone(),
                    fragment.interface_version.clone(),
                );
                let interface_key = interface_key_string(
                    &spec_str_lossy(&key.0),
                    &spec_str_lossy(&key.1),
                );
                // Malformed producer output fails loudly even on candidates
                // that will be shaken; conflict detection, by contrast, is
                // deferred to anchored interfaces post-shake (see
                // `InterfaceBucket::candidates`).
                for exn in fragment.throws.iter() {
                    validate_type(&exn.ty)?;
                }
                let mut emits = fragment.emits.to_vec();
                for name in &emits {
                    if spec_str(name)?.is_empty() {
                        return Err(SpecToolError::InvalidRawFragment(
                            format!(
                                "{interface_key}: interface emits entries must be non-empty"
                            ),
                        ));
                    }
                }
                emits.sort();
                emits.dedup();

                let candidate = (fragment.throws.to_vec(), emits);
                let bucket = interfaces.entry(key).or_default();
                if !bucket.candidates.contains(&candidate) {
                    bucket.candidates.push(candidate);
                }
            }
            SpecFragment::SpecMethod {
                method_fragment: fragment,
            } => {
                validate_interface_key(
                    spec_str(&fragment.interface_name)?,
                    spec_str(&fragment.interface_version)?,
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
                validate_type_def(type_def)?;
                let symbol = type_def_symbol(type_def)?.clone();
                spec_str(&symbol)?;
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
            SpecFragment::ExportedSpecTypeDef { type_def } => {
                validate_type_def(type_def)?;
                let symbol = type_def_symbol(type_def)?.clone();
                spec_str(&symbol)?;
                exported_roots.insert(symbol.clone());
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
            // `SpecEvent` fragments are candidates referenced by interface
            // `emits` lists; they survive only if a surviving (anchored)
            // interface lists their name in `emits`.
            // `ExportedSpecEvent` fragments come from `#[event_spec(export)]`
            // and force inclusion unconditionally.
            SpecFragment::SpecEvent { event_def }
            | SpecFragment::ExportedSpecEvent { event_def } => {
                validate_event(event_def)?;
                if matches!(fragment, SpecFragment::ExportedSpecEvent { .. }) {
                    exported_events.insert(event_def.name.clone());
                }
                let defs = events.entry(event_def.name.clone()).or_default();
                if !defs.iter().any(|existing| existing == event_def) {
                    defs.push(event_def.clone());
                }
            }
            SpecFragment::InterfaceAnchor { interface_anchor } => {
                validate_interface_key(
                    spec_str(&interface_anchor.interface_name)?,
                    spec_str(&interface_anchor.interface_version)?,
                )?;
                anchored_interfaces.insert((
                    interface_anchor.interface_name.clone(),
                    interface_anchor.interface_version.clone(),
                ));
            } // `SpecFragment` is SDK-owned and matched exhaustively: a new
              // fragment kind is a compile error here, and an unknown
              // discriminant in raw bytes fails XDR decode in
              // `parse_raw_fragments` before reaching this match.
        }
    }

    if interfaces.is_empty() {
        return Err(SpecToolError::NoSpecFragments);
    }

    // Typedef conflicts are checked in `collect_named_types`, which sees only
    // defs reachable from the retained spec. A def conflict among shaken
    // (unreachable) typedefs — e.g. two versions of an interface crate linked
    // by a caller that implements neither — must not fail the build, matching
    // the event-def policy above.

    // Spec shaking: keep only interfaces that were anchored (by an impl, or by
    // `export_interface_spec!` in an interface carrier). A trait's interface
    // candidates that nothing in this artifact anchors are dropped.
    //
    // While iterating surviving interfaces, collect the event names they emit
    // so we can shake unreferenced event defs in the next step.
    let mut retained_event_names = exported_events;
    let interfaces = interfaces
        .into_iter()
        .filter(|((name, vers), _)| {
            anchored_interfaces.contains(&(name.clone(), vers.clone()))
        })
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
                        spec_str_lossy(&name),
                        spec_str_lossy(&vers),
                        spec_str_lossy(&method.name),
                    )));
                }
                deduped_methods.push(method);
            }

            // Interface-metadata conflicts (throws, emits, exception
            // selectors) are only errors for anchored interfaces: candidates
            // of shaken interfaces never got here. Throws are compared in
            // canonical form so fragment ordering cannot cause spurious
            // conflicts.
            let interface_key = interface_key_string(
                &spec_str_lossy(&name),
                &spec_str_lossy(&vers),
            );
            let mut candidates = bucket.candidates.into_iter();
            let (first_throws, emits) =
                candidates.next().ok_or_else(|| {
                    SpecToolError::MissingInterfaceFragment(
                        interface_key.clone(),
                    )
                })?;
            let throws =
                canonicalize_exception_list(&interface_key, &first_throws)?;
            for (other_throws, other_emits) in candidates {
                let other_throws = canonicalize_exception_list(
                    &interface_key,
                    &other_throws,
                )?;
                if other_throws != throws {
                    return Err(SpecToolError::ConflictingInterfaceThrows(
                        interface_key,
                    ));
                }
                if other_emits != emits {
                    return Err(SpecToolError::ConflictingInterfaceEmits(
                        interface_key,
                    ));
                }
            }
            retained_event_names.extend(emits);

            Ok(ContractSpecInterface {
                name,
                vers,
                throws: throws.try_into().unwrap(),
                methods: deduped_methods.try_into().unwrap(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Event shaking: a def survives only if a surviving interface's `emits`
    // references it, or it was force-exported via `#[event_spec(export)]`.
    // Conflicts among shaken defs are deliberately ignored — an unimplemented
    // interface candidate must not fail an unrelated contract's build.
    let events = retained_event_names
        .into_iter()
        .map(|name| {
            let defs = events.remove(&name).ok_or_else(|| {
                SpecToolError::MissingEventDef(
                    spec_str_lossy(&name).into_owned(),
                )
            })?;
            if defs.len() > 1 {
                return Err(SpecToolError::ConflictingEventDef(
                    spec_str_lossy(&name).into_owned(),
                ));
            }
            Ok(defs.into_iter().next().unwrap())
        })
        .collect::<Result<Vec<_>, _>>()?;

    let type_defs = resolve_reachable_type_defs(
        &interfaces,
        &events,
        &type_defs,
        &exported_roots,
    )?;
    Ok(ContractSpec {
        interfaces: interfaces.try_into().unwrap(),
        type_defs: type_defs.try_into().unwrap(),
        events: events.try_into().unwrap(),
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
        "events": spec.events.iter().map(event_def_to_json).collect::<Vec<_>>(),
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
    spec_str(&method.name)?;
    for arg in method.args.iter() {
        spec_str(&arg.name)?;
        validate_type(&arg.ty)?;
    }
    validate_type(&method.return_type)?;
    Ok(())
}

/// Rejects types containing variants this tool version does not know.
///
/// Every type entering [`canonicalize_spec`] passes through here (method
/// signatures, throws, typedef bodies, event fields), so the later
/// total-function paths — sort-key rendering and JSON output — can rely on
/// only known variants reaching them.
fn validate_type(ty: &ContractType) -> Result<(), SpecToolError> {
    match ty {
        ContractType::Option { option_inner } => validate_type(option_inner),
        ContractType::Vector { vector } => validate_type(&vector.element),
        ContractType::Array { array } => validate_type(&array.element),
        ContractType::Tuple { tuple } => {
            tuple.iter().try_for_each(validate_type)
        }
        ContractType::SignedToken {
            signed_token_preimage,
        } => match &signed_token_preimage.preimage_type {
            Some(preimage_type) => validate_type(preimage_type),
            // An erased (opaque) preimage.
            None => Ok(()),
        },
        ContractType::Named { named } => spec_str(named).map(|_| ()),
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
        | ContractType::Address => Ok(()),
        _ => Err(SpecToolError::UnsupportedVariant(
            "unknown ContractType variant".to_string(),
        )),
    }
}

fn validate_type_def(type_def: &ContractTypeDef) -> Result<(), SpecToolError> {
    match type_def {
        ContractTypeDef::TypeDefStruct { struct_def } => struct_def
            .fields
            .iter()
            .try_for_each(|field| validate_type(&field.ty)),
        ContractTypeDef::TypeDefEnum { .. } => Ok(()),
        ContractTypeDef::TypeDefUnion { union_def } => union_def
            .cases
            .iter()
            .try_for_each(|case| validate_union_case_type(&case.case_type)),
        _ => Err(SpecToolError::UnsupportedVariant(
            "unknown ContractTypeDef variant".to_string(),
        )),
    }
}

fn validate_union_case_type(
    case_type: &ContractUnionCaseType,
) -> Result<(), SpecToolError> {
    match case_type {
        ContractUnionCaseType::UnionCaseVoid => Ok(()),
        ContractUnionCaseType::UnionCaseTuple { tuple } => {
            tuple.iter().try_for_each(validate_type)
        }
        ContractUnionCaseType::UnionCaseStruct { struct_fields } => {
            struct_fields
                .iter()
                .try_for_each(|field| validate_type(&field.ty))
        }
        _ => Err(SpecToolError::UnsupportedVariant(
            "unknown ContractUnionCaseType variant".to_string(),
        )),
    }
}

fn validate_event(event: &ContractEventDef) -> Result<(), SpecToolError> {
    if event.name.is_empty() {
        return Err(SpecToolError::InvalidRawFragment(
            "event fragments must include an event name".to_string(),
        ));
    }
    spec_str(&event.name)?;
    // Topic 0 is reserved for the event signature.
    let max_user_topics = (sdk::EVENT_MAX_TOPICS - 1) as usize;
    if event.topics.len() > max_user_topics {
        return Err(SpecToolError::InvalidRawFragment(format!(
            "event {} declares {} topics, but at most {max_user_topics} are allowed",
            spec_str_lossy(&event.name),
            event.topics.len(),
        )));
    }
    for field in event.topics.iter().chain(event.fields.iter()) {
        validate_type(&field.ty)?;
    }
    Ok(())
}

fn canonicalize_exception_list(
    interface_key: &str,
    types: &[ContractExceptionType],
) -> Result<Vec<ContractExceptionType>, SpecToolError> {
    // Validate before sorting: the sort key below assumes known variants.
    for exn in types {
        validate_type(&exn.ty)?;
    }
    let mut types = types.to_vec();
    // Canonical order is by selector, since the selector is the key clients
    // decode exceptions by.
    types.sort_by(|left, right| {
        left.selector.cmp(&right.selector).then_with(|| {
            canonical_type_key(&left.ty).cmp(&canonical_type_key(&right.ty))
        })
    });
    types.dedup();

    // After dedup, adjacent entries sharing a selector must differ in payload
    // type — that makes selector-based exception decoding ambiguous.
    for pair in types.windows(2) {
        if pair[0].selector == pair[1].selector {
            return Err(SpecToolError::ConflictingExceptionSelector(format!(
                "{interface_key}: selector 0x{:016x} maps to multiple \
                 exception types",
                pair[0].selector
            )));
        }
    }

    Ok(types)
}

fn resolve_reachable_type_defs(
    interfaces: &[ContractSpecInterface],
    events: &[ContractEventDef],
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

    for event in events {
        for field in event.topics.iter().chain(event.fields.iter()) {
            collect_named_types(&field.ty, defs, &mut resolved, &mut pending)?;
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
            Some(_) => {
                return Err(SpecToolError::UnsupportedVariant(
                    "unknown ContractTypeDef variant".to_string(),
                ));
            }
            None => {
                return Err(SpecToolError::MissingTypeDef(
                    spec_str_lossy(&symbol).into_owned(),
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
        ContractType::SignedToken {
            signed_token_preimage,
        } => {
            if let Some(preimage_type) = &signed_token_preimage.preimage_type {
                collect_named_types(preimage_type, defs, resolved, pending)?;
            }
        }
        ContractType::Named { named } => {
            spec_str(named)?;
            let Some(candidates) = defs.get(named) else {
                return Err(SpecToolError::MissingTypeDef(
                    spec_str_lossy(named).into_owned(),
                ));
            };
            if candidates.len() > 1 {
                return Err(SpecToolError::ConflictingTypeDef(
                    spec_str_lossy(named).into_owned(),
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
        // A variant this tool doesn't know about must fail loudly, not
        // silently drop its typedefs from the spec.
        _ => {
            return Err(SpecToolError::UnsupportedVariant(
                "unknown ContractType variant".to_string(),
            ));
        }
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
        _ => {
            return Err(SpecToolError::UnsupportedVariant(
                "unknown ContractUnionCaseType variant".to_string(),
            ));
        }
    }
    Ok(())
}

fn canonical_method_key(method: &ContractSpecMethod) -> String {
    let mut key = spec_str_lossy(&method.name).into_owned();
    key.push('|');
    for arg in method.args.iter() {
        key.push_str(&spec_str_lossy(&arg.name));
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
            format!("named({})", spec_str_lossy(named))
        }
        ContractType::SignedToken {
            signed_token_preimage,
        } => match &signed_token_preimage.preimage_type {
            Some(preimage_type) => {
                format!("signed_token({})", canonical_type_key(preimage_type))
            }
            None => "signed_token(opaque)".to_string(),
        },
        // Unreachable for canonicalized input: `validate_type` rejects
        // unknown variants at fragment ingestion, before any sort key is
        // rendered. Kept as a loud backstop for direct callers.
        _ => panic!("unknown ContractType variant"),
    }
}

fn type_def_symbol(
    type_def: &ContractTypeDef,
) -> Result<&ContractSpecString, SpecToolError> {
    match type_def {
        ContractTypeDef::TypeDefStruct { struct_def } => Ok(&struct_def.name),
        ContractTypeDef::TypeDefEnum { enum_def } => Ok(&enum_def.name),
        ContractTypeDef::TypeDefUnion { union_def } => Ok(&union_def.name),
        _ => Err(SpecToolError::UnsupportedVariant(
            "unknown ContractTypeDef variant".to_string(),
        )),
    }
}

fn interface_key_string(name: &str, version: &str) -> String {
    format!("{name}@{version}")
}

fn interface_to_json(interface: &ContractSpecInterface) -> serde_json::Value {
    serde_json::json!({
        "name": spec_str_lossy(&interface.name),
        "version": spec_str_lossy(&interface.vers),
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
        "name": spec_str_lossy(&method.name),
        "args": method.args.iter().map(field_to_json).collect::<Vec<_>>(),
        "return_type": type_to_json(&method.return_type),
    })
}

fn field_to_json(field: &ContractField) -> serde_json::Value {
    serde_json::json!({
        "name": spec_str_lossy(&field.name),
        "type": type_to_json(&field.ty),
    })
}

pub fn event_def_to_json(event: &ContractEventDef) -> serde_json::Value {
    let signature: String =
        event.signature.iter().map(|b| format!("{b:02x}")).collect();
    serde_json::json!({
        "name": spec_str_lossy(&event.name),
        "signature": signature,
        "topics": event.topics.iter().map(field_to_json).collect::<Vec<_>>(),
        "fields": event.fields.iter().map(field_to_json).collect::<Vec<_>>(),
    })
}

pub fn type_to_json(ty: &ContractType) -> serde_json::Value {
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
            json!({"kind": "vec", "element": type_to_json(element), "max_len": vector.max_len})
        }
        ContractType::Tuple { tuple } => {
            json!({"kind": "tuple", "items": tuple.iter().map(type_to_json).collect::<Vec<_>>()})
        }
        ContractType::Named { named } => {
            json!({"kind": "named", "name": spec_str_lossy(named)})
        }
        ContractType::Array { array } => {
            let element = &array.element;
            json!({"kind": "array", "len": array.len, "element": type_to_json(element)})
        }
        ContractType::SignedToken {
            signed_token_preimage,
        } => {
            // `preimage` is `null` for an erased (opaque) payload.
            json!({
                "kind": "signed_token",
                "preimage": signed_token_preimage
                    .preimage_type
                    .as_deref()
                    .map(type_to_json),
            })
        }
        // Unreachable for canonicalized input (see `validate_type`); loud
        // backstop for direct callers of this pub fn.
        _ => panic!("unknown ContractType variant"),
    }
}

pub fn type_def_to_json(type_def: &ContractTypeDef) -> serde_json::Value {
    use serde_json::json;
    match type_def {
        ContractTypeDef::TypeDefStruct { struct_def } => json!({
            "kind": "struct",
            "name": spec_str_lossy(&struct_def.name),
            "fields": struct_def.fields.iter().map(field_to_json).collect::<Vec<_>>(),
        }),
        ContractTypeDef::TypeDefEnum { enum_def } => json!({
            "kind": "enum",
            "name": spec_str_lossy(&enum_def.name),
            "variants": enum_def.variants.iter().map(|v| json!({"name": spec_str_lossy(&v.name), "value": v.value})).collect::<Vec<_>>(),
        }),
        ContractTypeDef::TypeDefUnion { union_def } => json!({
            "kind": "union",
            "name": spec_str_lossy(&union_def.name),
            "cases": union_def.cases.iter().map(|c| json!({
                "name": spec_str_lossy(&c.name),
                "discriminant": c.discriminant,
                "case_type": union_case_type_to_json(&c.case_type),
            })).collect::<Vec<_>>(),
        }),
        // Unreachable for canonicalized input (see `validate_type_def`);
        // loud backstop for direct callers of this pub fn.
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
        // Unreachable for canonicalized input (see
        // `validate_union_case_type`); loud backstop for direct callers.
        _ => panic!("unknown ContractUnionCaseType variant"),
    }
}
