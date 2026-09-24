use std::collections::BTreeMap;

pub mod describe;
pub mod raw;

pub use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractEventDef,
    ContractExceptionType, ContractField, ContractSpec, ContractSpecInterface,
    ContractSpecMethod, ContractSpecString, ContractStructDef, ContractType,
    ContractTypeDef, ContractUnionCase, ContractUnionCaseType,
    ContractUnionDef, SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};
pub use raw::{
    FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME,
    RAW_SECTION_PREFIX, RAW_SPEC_FORMAT, SpecSection, classify_section,
    parse_raw_fragments,
};

/// Decode a spec string, rejecting non-UTF-8 bytes.
///
/// `ContractSpecString` is raw bytes on the wire (XDR decode performs no
/// UTF-8 validation), so a malformed artifact can carry arbitrary bytes here.
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
    /// A `throws` entry's payload type reaches a `SignedToken`. Abort frames
    /// carry no auth-token handles, so the token could only ever decode to
    /// an uninitialized sentinel.
    ExceptionCarriesAuthToken(String),
    /// Multiple conflicting method definitions share the same name in an interface.
    ConflictingMethod(String),
    /// Multiple conflicting event definitions exist for the same event name.
    ConflictingEventDef(String),
    /// Two retained events share a signature. The signature is topic 0 of
    /// every emitted event, so a consumer cannot tell them apart on the wire
    /// even though the spec names them separately.
    ConflictingEventSignature(String),
    /// Multiple interface fragments declare different `emits` for the same interface.
    ConflictingInterfaceEmits(String),
    /// One artifact anchors two different declarers (traits, or a trait and
    /// a bare-impl contract) under one interface key. The key is all the
    /// wire carries — `interface_id` and every entrypoint id hash only the
    /// interface name and major version — so callers cannot tell the two
    /// interfaces apart.
    ConflictingInterfaceDeclarers(String),
    /// An interface's `emits` references an event with no definition fragment.
    MissingEventDef(String),
    /// The spec contains a variant this tool version does not understand
    /// (e.g. it was produced by a newer toolchain). Rejecting it loudly
    /// beats silently dropping or mangling parts of the spec.
    UnsupportedVariant(String),
    /// The artifact's raw spec section is a newer wire format than this build
    /// understands. See [`check_raw_spec_format`].
    RawSpecFormatTooNew { found: u32, supported: u32 },
    /// The artifact's raw spec section is an older wire format than this
    /// build understands. See [`check_raw_spec_format`].
    RawSpecFormatTooOld { found: u32, supported: u32 },
}

/// Check an artifact's raw spec wire format against this build's.
///
/// The raw section is an unframed XDR stream, so a version disagreement
/// cannot be recovered from once decoding starts — it desynchronizes and
/// every later fragment decodes as garbage. Both directions are therefore
/// rejected up front, and each names the two versions and the way out: the
/// two sides are released independently (the SDK compiled into the contract
/// versus the `groundhog-spec-tool` linked into the CLI), so either can be
/// the stale one and the fix differs.
pub fn check_raw_spec_format(found: u32) -> Result<(), SpecToolError> {
    match found.cmp(&RAW_SPEC_FORMAT) {
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Greater => {
            Err(SpecToolError::RawSpecFormatTooNew {
                found,
                supported: RAW_SPEC_FORMAT,
            })
        }
        std::cmp::Ordering::Less => Err(SpecToolError::RawSpecFormatTooOld {
            found,
            supported: RAW_SPEC_FORMAT,
        }),
    }
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
            Self::ExceptionCarriesAuthToken(msg) => {
                write!(f, "exception payload carries an auth token: {msg}")
            }
            Self::ConflictingMethod(msg) => {
                write!(f, "conflicting method definitions for {msg}")
            }
            Self::ConflictingEventDef(msg) => {
                write!(f, "conflicting event definitions for {msg}")
            }
            Self::ConflictingEventSignature(msg) => {
                write!(f, "events share an event signature: {msg}")
            }
            Self::ConflictingInterfaceEmits(msg) => {
                write!(f, "conflicting emits metadata for interface {msg}")
            }
            Self::ConflictingInterfaceDeclarers(msg) => {
                write!(f, "interface declared by more than one item: {msg}")
            }
            Self::MissingEventDef(msg) => {
                write!(f, "missing event definition for {msg}")
            }
            Self::UnsupportedVariant(msg) => {
                write!(f, "unsupported spec variant: {msg}")
            }
            Self::RawSpecFormatTooNew { found, supported } => write!(
                f,
                "this contract was built with a newer Groundhog SDK than \
                 this tool understands (contract raw spec format {found}, \
                 this tool supports {supported}). Upgrade the groundhog CLI, \
                 or rebuild the contract against an SDK matching it.",
            ),
            Self::RawSpecFormatTooOld { found, supported } => write!(
                f,
                "this contract was built with an older Groundhog SDK than \
                 this tool understands (contract raw spec format {found}, \
                 this tool supports {supported}). Rebuild the contract \
                 against a newer SDK, or use a groundhog CLI matching the \
                 SDK it was built with.",
            ),
        }
    }
}

impl std::error::Error for SpecToolError {}

/// Everything the fragments say about one interface key, from every declarer
/// that uses it.
///
/// Unrelated interface crates can collide on a key (trait-ident name,
/// crate-version version), so a bucket may hold several declarers' fragments.
/// Each entry carries its `declared_by`, and only the anchored declarer's
/// survive shaking: a caller linking a collider it does not implement must
/// neither fail on the collider's disagreeing metadata nor inherit its
/// methods.
#[derive(Default)]
struct InterfaceBucket {
    /// Distinct (declared_by, throws, emits) candidates; emits
    /// pre-canonicalized (sorted, deduped), throws kept raw. Conflicts among
    /// one declarer's candidates — including selector conflicts within a
    /// throws list — are checked after shaking.
    candidates: Vec<(
        ContractSpecString,
        Vec<ContractExceptionType>,
        Vec<ContractSpecString>,
    )>,
    methods: Vec<(ContractSpecString, ContractSpecMethod)>,
}

/// Assembles raw spec fragments into a canonical [`ContractSpec`].
///
/// Deduplicates methods and type definitions, validates interface keys,
/// and resolves only the type definitions reachable from interface signatures.
pub fn canonicalize_spec(
    raw: &[SpecFragment],
) -> Result<ContractSpec, SpecToolError> {
    canonicalize_spec_inner(raw, None)
}

/// Like [`canonicalize_spec`], but additionally shakes methods with no
/// corresponding exported entrypoint.
///
/// `exported_fns` is the module's set of **entrypoint export names** — the
/// 16-hex uppercase rendering of the dispatch ID
/// ([`groundhog_contract_util::entrypoint_export_name`]). Callers must map
/// their dispatch-table method IDs through `entrypoint_export_name` — NOT
/// pass ELF symbol names (rv32 wrapper symbols carry the human-readable ID
/// preimage, which never matches; passing them would silently disable
/// shaking).
///
/// For each anchored interface, each method's expected entrypoint export
/// name is computed with the same `contract-util` functions the macro
/// codegen uses; methods whose entrypoint is absent are dropped.
///
/// This is a second line of defense against phantom methods — methods
/// contributed by a *different* trait candidate that collides on the
/// interface key (default name = trait ident, default version = crate
/// version). The first, and the one that does the work for artifacts built
/// by a current SDK, is at emission: only a site that anchors an interface
/// emits its `Method` fragments, so a trait nothing implements contributes
/// none. Passing exports additionally catches an artifact built by an older
/// SDK, where the fragments still carried unanchored candidates.
///
/// Carrier rule: an EMPTY `exported_fns` set disables shaking entirely.
/// Spec-carrier binaries (`export_interface_spec!`) export no functions at
/// all and must still publish complete interfaces. The exemption is
/// deliberately module-wide rather than per-interface: an anchored
/// interface with zero real methods (e.g. an implemented marker trait
/// whose key a collider squats) must not smuggle phantom methods through.
pub fn canonicalize_spec_with_exports(
    raw: &[SpecFragment],
    exported_fns: &std::collections::BTreeSet<String>,
) -> Result<ContractSpec, SpecToolError> {
    canonicalize_spec_inner(raw, Some(exported_fns))
}

fn canonicalize_spec_inner(
    raw: &[SpecFragment],
    exported_fns: Option<&std::collections::BTreeSet<String>>,
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
    //
    // Keyed by interface key, holding every declarer anchored under it. More
    // than one is an error (see `ConflictingInterfaceDeclarers`).
    let mut anchored_interfaces = BTreeMap::<
        (ContractSpecString, ContractSpecString),
        std::collections::BTreeSet<ContractSpecString>,
    >::new();
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

                validate_declared_by(&interface_key, &fragment.declared_by)?;
                let candidate = (
                    fragment.declared_by.clone(),
                    fragment.throws.to_vec(),
                    emits,
                );
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
                validate_declared_by(
                    &interface_key_string(
                        &spec_str_lossy(&fragment.interface_name),
                        &spec_str_lossy(&fragment.interface_version),
                    ),
                    &fragment.declared_by,
                )?;

                let key = (
                    fragment.interface_name.clone(),
                    fragment.interface_version.clone(),
                );
                interfaces.entry(key).or_default().methods.push((
                    fragment.declared_by.clone(),
                    fragment.method.clone(),
                ));
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
                validate_declared_by(
                    &interface_key_string(
                        &spec_str_lossy(&interface_anchor.interface_name),
                        &spec_str_lossy(&interface_anchor.interface_version),
                    ),
                    &interface_anchor.declared_by,
                )?;
                anchored_interfaces
                    .entry((
                        interface_anchor.interface_name.clone(),
                        interface_anchor.interface_version.clone(),
                    ))
                    .or_default()
                    .insert(interface_anchor.declared_by.clone());
            } // `SpecFragment` is SDK-owned and matched exhaustively: a new
              // fragment kind is a compile error here, and an unknown
              // discriminant in raw bytes fails XDR decode in
              // `parse_raw_fragments` before reaching this match.
        }
    }

    // Whether any spec content exists is decided AFTER shaking (see the
    // post-shake check below), not on the candidate map here: linked
    // interface crates make the candidate map non-empty even for artifacts
    // whose spec shakes to nothing, and export-only artifacts have spec
    // content with no interface candidates at all.

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
    //
    // An artifact that anchors two declarers under one key implements two
    // interfaces the wire cannot tell apart: they share an interface id, and
    // any method signature they share maps to one entrypoint id. Refuse it
    // rather than publishing one interface with both method sets.
    //
    // Compared per (name, MAJOR), not per key: the key carries the full
    // version, but the wire identity hashes only the major, so `Token@1.0.0`
    // and `Token@1.1.0` from two declarers collide on the wire while landing
    // in separate buckets here.
    let mut declarers_by_wire_identity = BTreeMap::<
        (String, String),
        std::collections::BTreeSet<&ContractSpecString>,
    >::new();
    for ((name, vers), declarers) in &anchored_interfaces {
        declarers_by_wire_identity
            .entry((
                spec_str_lossy(name).into_owned(),
                groundhog_contract_util::semver::major_version_from_string(
                    spec_str(vers)?,
                ),
            ))
            .or_default()
            .extend(declarers);
    }
    for ((name, major), declarers) in &declarers_by_wire_identity {
        if declarers.len() > 1 {
            let named = declarers
                .iter()
                .map(|declarer| spec_str_lossy(declarer).into_owned())
                .collect::<Vec<_>>()
                .join(" and ");
            return Err(SpecToolError::ConflictingInterfaceDeclarers(
                format!(
                    "{} is declared by both {named}; its interface id and \
                     entrypoint ids hash only the interface name and major \
                     version, so callers cannot tell them apart — give one \
                     a distinct name with #[contract(interface = \"...\")]",
                    interface_key_string(name, major),
                ),
            ));
        }
    }

    let mut retained_event_names = exported_events;
    let interfaces = interfaces
        .into_iter()
        .filter_map(|(key, mut bucket)| {
            // Exactly one declarer per anchored key, checked above. Keep only
            // its fragments: a collider's are shaken like any unanchored
            // interface's.
            let declarer = anchored_interfaces.get(&key)?.first()?.clone();
            bucket.candidates.retain(|(by, _, _)| *by == declarer);
            let methods = std::mem::take(&mut bucket.methods);
            bucket.methods = methods
                .into_iter()
                .filter(|(by, _)| *by == declarer)
                .collect();
            Some((key, bucket))
        })
        .map(|((name, vers), bucket)| {
            let mut methods = bucket
                .methods
                .into_iter()
                .map(|(_, method)| method)
                .collect::<Vec<_>>();
            methods.sort_by(|left, right| {
                left.name.cmp(&right.name).then_with(|| {
                    canonical_method_key(left)
                        .cmp(&canonical_method_key(right))
                })
            });
            let mut deduped_methods: Vec<ContractSpecMethod> =
                Vec::with_capacity(methods.len());
            for method in methods {
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

            // Method shaking by export cross-reference (see
            // `canonicalize_spec_with_exports`): drop methods whose
            // entrypoint the module does not export — phantom methods from
            // a colliding trait candidate. Skipped only when the module
            // exports no functions at all (a spec-carrier binary), which
            // the caller signals with an empty set: the exemption is
            // module-wide, never per-interface, so an anchored interface
            // with zero real methods cannot smuggle a collider's phantoms
            // through.
            if let Some(exports) = exported_fns
                && !exports.is_empty()
            {
                let iface_name = spec_str(&name)?.to_string();
                let iface_vers = spec_str(&vers)?.to_string();
                deduped_methods = deduped_methods
                    .into_iter()
                    .map(|method| {
                        let export_name =
                            groundhog_contract_util::entrypoint_export_name_for(
                                Some(&iface_name),
                                &iface_vers,
                                &groundhog_contract_util::NormalizedMethodSignature::new(
                                    spec_str(&method.name)?,
                                ),
                            );
                        Ok((exports.contains(&export_name), method))
                    })
                    .collect::<Result<Vec<_>, SpecToolError>>()?
                    .into_iter()
                    .filter_map(|(keep, method)| keep.then_some(method))
                    .collect();
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
            let (_, first_throws, emits) =
                candidates.next().ok_or_else(|| {
                    SpecToolError::MissingInterfaceFragment(
                        interface_key.clone(),
                    )
                })?;
            let throws =
                canonicalize_exception_list(&interface_key, &first_throws)?;
            for (_, other_throws, other_emits) in candidates {
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

    // Two retained events must not share a signature. The signature is topic 0
    // of every emitted event, so it is the only thing a consumer has to tell
    // events apart on the wire — and it is derived from (crate, MAJOR, ident),
    // which is less than the spec name distinguishes. Two events in different
    // modules of one crate therefore have distinct names and one signature:
    // the spec would promise a distinction the wire cannot deliver.
    //
    // Checked over retained events only, like every other conflict here: two
    // shaken candidates that disagree are nobody's problem.
    {
        let mut by_sig: BTreeMap<&[u8], &ContractSpecString> = BTreeMap::new();
        for event in &events {
            let sig: &[u8] = event.signature.as_ref();
            if let Some(first) = by_sig.insert(sig, &event.name) {
                return Err(SpecToolError::ConflictingEventSignature(
                    format!(
                        "{} and {} both hash to {}; they differ only in a module \
                     path, which the signature does not carry — rename one",
                        spec_str_lossy(first),
                        spec_str_lossy(&event.name),
                        sig.iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>(),
                    ),
                ));
            }
        }
    }

    let type_defs = resolve_reachable_type_defs(
        &interfaces,
        &events,
        &type_defs,
        &exported_roots,
    )?;

    reject_auth_tokens_in_throws(&interfaces, &type_defs)?;

    // Nothing anchored, nothing exported: the artifact has no spec. Report
    // NoSpecFragments so the caller can distinguish "raw contract whose
    // linked fragments all shook out" (benign, no spec section) from "a
    // contract missing its #[contract] macro" (no entrypoints — hard error
    // in rewrite). A real #[contract] contract always anchors its own
    // interface, so a well-formed contract can never reach this.
    if interfaces.is_empty() && events.is_empty() && type_defs.is_empty() {
        return Err(SpecToolError::NoSpecFragments);
    }

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
    // Same acceptance contract as the macro-side validator
    // (macros/common/util.rs validate_version_string), and the same strict
    // parser the shaking path runs when computing entrypoint IDs: canonical
    // semver triplets, optionally with -PRERELEASE/+BUILD suffixes.
    // Non-canonical strings ("01.0.0", components beyond u64) are rejected
    // here so crafted or corrupt raw sections produce an error, never a
    // panic in contract-util's panicking parser.
    if groundhog_contract_util::semver::try_major_version_from_string(
        interface_version,
    )
    .is_none()
    {
        return Err(SpecToolError::InvalidRawFragment(format!(
            "interface version must be a semver triplet like \"1.0.0\" \
             (optionally with -PRERELEASE and +BUILD suffixes), got \
             {interface_version}"
        )));
    }
    Ok(())
}

/// Reject a missing or malformed `declared_by`: every interface-keyed
/// fragment names its declarer, and shaking keys on it.
fn validate_declared_by(
    interface_key: &str,
    declared_by: &ContractSpecString,
) -> Result<(), SpecToolError> {
    if spec_str(declared_by)?.is_empty() {
        return Err(SpecToolError::InvalidRawFragment(format!(
            "{interface_key}: interface fragments must name their declarer"
        )));
    }
    Ok(())
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
/// Reject any `throws(...)` payload whose type graph reaches a
/// `SignedToken`.
///
/// A signed token is carried out of band: the handle travels in the
/// auth-token list the host returns beside a *successful* return, while the
/// token itself occupies zero wire bytes. Abort frames have no such list —
/// the host refuses to return auth tokens on a revert frame — so a token
/// reached through an exception payload can never be given a real handle.
/// Decoding one yields an uninitialized sentinel: a value shaped like an
/// auth token that faults on every use, at a moment the throwing contract
/// chooses.
///
/// There is no encoding that makes this work, so it is rejected here rather
/// than diagnosed later. Checking the spec (instead of the Rust types)
/// means the rule holds for every SDK that emits a spec, costs nothing at
/// runtime, and imposes no trait bound on exception types.
fn reject_auth_tokens_in_throws(
    interfaces: &[ContractSpecInterface],
    type_defs: &[ContractTypeDef],
) -> Result<(), SpecToolError> {
    let defs: BTreeMap<&ContractSpecString, &ContractTypeDef> = type_defs
        .iter()
        .filter_map(|def| type_def_name(def).map(|name| (name, def)))
        .collect();

    for interface in interfaces {
        for exn in interface.throws.iter() {
            let mut visited = std::collections::BTreeSet::new();
            if type_reaches_auth_token(&exn.ty, &defs, &mut visited) {
                return Err(SpecToolError::ExceptionCarriesAuthToken(
                    format!(
                        "{}: exception with selector 0x{:016x} contains a \
                         signed token; abort frames carry no auth handles, \
                         so it could only decode to an uninitialized \
                         sentinel — return the token from a successful call \
                         instead",
                        spec_str_lossy(&interface.name),
                        exn.selector,
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// The declared name of a type def, or `None` for a variant this build does
/// not model. An unknown def is simply not resolvable here;
/// [`validate_type_def`] is what rejects it.
fn type_def_name(def: &ContractTypeDef) -> Option<&ContractSpecString> {
    match def {
        ContractTypeDef::TypeDefStruct { struct_def } => {
            Some(&struct_def.name)
        }
        ContractTypeDef::TypeDefEnum { enum_def } => Some(&enum_def.name),
        ContractTypeDef::TypeDefUnion { union_def } => Some(&union_def.name),
        _ => None,
    }
}

/// Whether `ty` reaches a `SignedToken`, resolving `Named` references
/// against `defs`.
///
/// `visited` guards the named-type recursion: contract types may be
/// recursive (a tree holding subtrees), and a token reachable only by
/// descending forever is not reachable at all, so revisiting a name in
/// progress contributes nothing.
fn type_reaches_auth_token(
    ty: &ContractType,
    defs: &BTreeMap<&ContractSpecString, &ContractTypeDef>,
    visited: &mut std::collections::BTreeSet<ContractSpecString>,
) -> bool {
    match ty {
        // The preimage is not inspected: the token itself is already
        // disqualifying, whatever it wraps.
        ContractType::SignedToken { .. } => true,
        ContractType::Option { option_inner } => {
            type_reaches_auth_token(option_inner, defs, visited)
        }
        ContractType::Vector { vector } => {
            type_reaches_auth_token(&vector.element, defs, visited)
        }
        ContractType::Array { array } => {
            type_reaches_auth_token(&array.element, defs, visited)
        }
        ContractType::Tuple { tuple } => tuple
            .iter()
            .any(|inner| type_reaches_auth_token(inner, defs, visited)),
        ContractType::Named { named } => {
            if !visited.insert(named.clone()) {
                return false;
            }
            // A dangling name cannot be walked. `resolve_reachable_type_defs`
            // has already run, so it would have reported the missing def.
            defs.get(named).is_some_and(|def| {
                type_def_reaches_auth_token(def, defs, visited)
            })
        }
        _ => false,
    }
}

/// Whether a type def's fields (or union-case payloads) reach a
/// `SignedToken`. Enums are plain discriminants, so they carry nothing.
fn type_def_reaches_auth_token(
    def: &ContractTypeDef,
    defs: &BTreeMap<&ContractSpecString, &ContractTypeDef>,
    visited: &mut std::collections::BTreeSet<ContractSpecString>,
) -> bool {
    match def {
        ContractTypeDef::TypeDefStruct { struct_def } => struct_def
            .fields
            .iter()
            .any(|field| type_reaches_auth_token(&field.ty, defs, visited)),
        ContractTypeDef::TypeDefUnion { union_def } => {
            union_def.cases.iter().any(|case| match &case.case_type {
                ContractUnionCaseType::UnionCaseVoid => false,
                ContractUnionCaseType::UnionCaseTuple { tuple } => tuple
                    .iter()
                    .any(|ty| type_reaches_auth_token(ty, defs, visited)),
                ContractUnionCaseType::UnionCaseStruct { struct_fields } => {
                    struct_fields.iter().any(|field| {
                        type_reaches_auth_token(&field.ty, defs, visited)
                    })
                }
                _ => false,
            })
        }
        _ => false,
    }
}

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
        | ContractType::U256
        | ContractType::I256
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
        | ContractType::U256
        | ContractType::I256
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
        ContractType::U256 => "u256".to_string(),
        ContractType::I256 => "i256".to_string(),
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
        ContractType::U256 => json!({"kind": "u256"}),
        ContractType::I256 => json!({"kind": "i256"}),
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
