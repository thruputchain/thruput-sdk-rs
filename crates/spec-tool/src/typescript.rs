//! Spec names the CLI's TypeScript codegen cannot bind.
//!
//! `groundhog codegen ts` nests each declared type and event under the path
//! its id maps to, `crate.vMAJOR.module.Name`, and keeps enum variants and
//! event descriptors in one object per crate. Some names cannot be written
//! that way. The contract still builds and deploys, so [`binding_warnings`]
//! reports them when the spec is finalized, before anyone asks for bindings.
//! The rules mirror the codegen's; change the two together.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use crate::{
    ContractSpec, ContractSpecId, ContractTypeDef, spec_id_lossy,
    spec_str_lossy,
};

/// The prefix the codegen keeps for the names it declares itself.
const RESERVED_PREFIX: &str = "__gh_";

/// Reasons `groundhog codegen ts` would refuse `spec`, one message each.
///
/// A message names the declarations involved, the way the other spec
/// diagnostics do, and what to rename. It reads as the tail of "cannot
/// generate TypeScript bindings: …", which is how the codegen reports it.
pub fn binding_warnings(spec: &ContractSpec) -> Vec<String> {
    let declared = declarations(spec);
    let mut warnings = Vec::new();
    for decl in &declared {
        path_warnings(decl, &mut warnings);
    }
    for iface in spec.interfaces.iter() {
        interface_warnings(&spec_str_lossy(&iface.name), &mut warnings);
    }
    namespace_collision_warnings(&declared, &mut warnings);
    crate_shadowing_warnings(&declared, &mut warnings);
    value_clash_warnings(&declared, &mut warnings);
    warnings
}

/// Where an id lands in the bindings: its namespaces, then its own name.
struct TsPath {
    namespaces: Vec<String>,
    name: String,
}

impl TsPath {
    /// The crate's `-` becomes `_`, the major nests as `vMAJOR`, and the
    /// module path drops its leading crate segment, as the codegen maps it.
    fn of(id: &ContractSpecId) -> Self {
        let krate = spec_str_lossy(&id.crate_name).replace('-', "_");
        let major = spec_str_lossy(&id.major).replace('.', "_");
        let module_path = spec_str_lossy(&id.module_path);
        let mut namespaces = vec![krate.clone(), format!("v{major}")];
        let mut segments = module_path
            .split("::")
            .filter(|segment| !segment.is_empty())
            .peekable();
        if segments.peek() == Some(&krate.as_str()) {
            segments.next();
        }
        namespaces.extend(segments.map(str::to_owned));
        TsPath {
            namespaces,
            name: spec_str_lossy(&id.declared).into_owned(),
        }
    }

    fn segments(&self) -> impl Iterator<Item = &String> {
        self.namespaces.iter().chain(std::iter::once(&self.name))
    }

    fn qualified(&self) -> String {
        self.segments()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(".")
    }
}

enum Kind<'a> {
    Struct,
    /// Its variant names.
    Enum(Vec<Cow<'a, str>>),
    Union,
    Event,
}

/// A declared type or event and the path the bindings give it.
struct Declaration<'a> {
    label: String,
    path: TsPath,
    kind: Kind<'a>,
}

impl Declaration<'_> {
    /// Whether it puts a value in its crate's object: enum variants or an
    /// event descriptor.
    fn has_value(&self) -> bool {
        matches!(self.kind, Kind::Enum(_) | Kind::Event)
    }
}

fn declarations(spec: &ContractSpec) -> Vec<Declaration<'_>> {
    let mut out = Vec::new();
    for def in spec.type_defs.iter() {
        let (word, id, kind) = match def {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                ("struct", &struct_def.name, Kind::Struct)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                let variants = enum_def
                    .variants
                    .iter()
                    .map(|variant| spec_str_lossy(&variant.name))
                    .collect();
                ("enum", &enum_def.name, Kind::Enum(variants))
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                ("union", &union_def.name, Kind::Union)
            }
            // A kind the codegen cannot emit either; it reports that itself.
            _ => continue,
        };
        out.push(Declaration {
            label: format!("{word} {}", spec_id_lossy(id)),
            path: TsPath::of(id),
            kind,
        });
    }
    for event in spec.events.iter() {
        out.push(Declaration {
            label: format!("event {}", spec_id_lossy(&event.name)),
            path: TsPath::of(&event.name),
            kind: Kind::Event,
        });
    }
    out
}

fn path_warnings(decl: &Declaration, out: &mut Vec<String>) {
    let path = &decl.path;
    if let Some(segment) = path.segments().find(|s| !is_ts_identifier(s)) {
        out.push(format!(
            "{} would be \"{}\" in the generated TypeScript, where \
             \"{segment}\" is not a valid identifier. Rename it.",
            decl.label,
            path.qualified()
        ));
    }
    if path.segments().any(|segment| segment == "globalThis") {
        out.push(format!(
            "{} has \"globalThis\" in its generated TypeScript path, where \
             it would shadow the global object the bindings reach built-in \
             types through. Rename the module or type.",
            decl.label
        ));
    }
    if let Some(segment) =
        path.segments().find(|s| s.starts_with(RESERVED_PREFIX))
    {
        out.push(format!(
            "generated TypeScript name \"{segment}\" for {} starts with \
             \"{RESERVED_PREFIX}\", which the generator reserves for the \
             names it declares itself. Rename it without that prefix.",
            decl.label
        ));
    }
}

/// An interface's name is declared at the top level of the bindings, and so
/// are the exception, event and method types named after it.
fn interface_warnings(name: &str, out: &mut Vec<String>) {
    if !is_ts_identifier(name) {
        out.push(format!(
            "interface \"{name}\" is not a valid TypeScript identifier. \
             Rename it."
        ));
    }
    if name == "globalThis" {
        out.push(
            "interface \"globalThis\" would shadow the global object the \
             bindings reach built-in types through. Rename it."
                .to_owned(),
        );
    }
    if name.starts_with(RESERVED_PREFIX) {
        out.push(format!(
            "interface \"{name}\" starts with \"{RESERVED_PREFIX}\", which \
             the generator reserves for the names it declares itself. \
             Rename it without that prefix."
        ));
    }
}

/// Each type, each event and each event's filter type takes a name in its
/// namespace, and no two can share one: an enum renamed after an event beside
/// it, say, or a struct named like an event's filter type.
fn namespace_collision_warnings(
    declared: &[Declaration],
    out: &mut Vec<String>,
) {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    // Types before events, as the codegen reserves them, so a collision
    // names the two in the same order the codegen's error does.
    let types = declared
        .iter()
        .filter(|decl| !matches!(decl.kind, Kind::Event));
    let events = declared
        .iter()
        .filter(|decl| matches!(decl.kind, Kind::Event));
    for decl in types.chain(events) {
        let qualified = decl.path.qualified();
        let mut claims = vec![(qualified.clone(), decl.label.clone())];
        if matches!(decl.kind, Kind::Event) {
            claims.push((
                format!("{qualified}Filter"),
                format!("the filter type of {}", decl.label),
            ));
        }
        for (name, source) in claims {
            match names.entry(name) {
                Entry::Occupied(existing) => out.push(format!(
                    "generated TypeScript value name \"{}\" collides between \
                     {} and {source}",
                    existing.key(),
                    existing.get()
                )),
                Entry::Vacant(slot) => {
                    slot.insert(source);
                }
            }
        }
    }
}

/// A named type is written from its crate's root down, and TypeScript
/// resolves that first segment to the nearest namespace of that name, so a
/// module named after a crate would take over references to its types.
fn crate_shadowing_warnings(declared: &[Declaration], out: &mut Vec<String>) {
    let crates: BTreeSet<&str> = declared
        .iter()
        .filter(|decl| !matches!(decl.kind, Kind::Event))
        .map(|decl| decl.path.namespaces[0].as_str())
        .collect();
    for decl in declared {
        if let Some(module) = decl.path.namespaces[1..]
            .iter()
            .find(|segment| crates.contains(segment.as_str()))
        {
            out.push(format!(
                "{} has \"{module}\" in its generated TypeScript path, which \
                 is also the name of a crate whose types the bindings write \
                 as \"{module}.…\". Inside that namespace the name would \
                 resolve to the module instead. Rename the module.",
                decl.label
            ));
        }
    }
}

/// Each value sits in its crate's object under its path, so an enum or event
/// beside a module of its own name shares that object with the module's
/// values. An enum's variants can, unless a key repeats; an event's
/// descriptor cannot hold them at all.
fn value_clash_warnings(declared: &[Declaration], out: &mut Vec<String>) {
    for decl in declared {
        let at: Vec<String> = decl.path.segments().cloned().collect();
        let path = decl.path.qualified();
        // The key each value under `decl`'s path needs in its object.
        let mut under = declared
            .iter()
            .filter(|inner| inner.has_value())
            .filter_map(|inner| {
                let key = if inner.path.namespaces == at {
                    &inner.path.name
                } else if inner.path.namespaces.starts_with(&at) {
                    &inner.path.namespaces[at.len()]
                } else {
                    return None;
                };
                Some((key, inner))
            });
        match &decl.kind {
            Kind::Event => {
                if let Some((key, inner)) = under.next() {
                    out.push(format!(
                        "{} and {} both need a value under \"{path}\" in the \
                         generated TypeScript: the event's descriptor is \
                         there, and it cannot also hold \"{key}\". Rename \
                         either, or move one out of that module.",
                        decl.label, inner.label
                    ));
                }
            }
            Kind::Enum(variants) => {
                let mut reported = BTreeSet::new();
                for (key, inner) in under {
                    if variants.iter().any(|variant| variant == key)
                        && reported.insert(key)
                    {
                        out.push(format!(
                            "{} has a variant \"{key}\", and {} also needs \
                             \"{path}.{key}\" in the generated TypeScript, \
                             where an enum's variants share one object with \
                             the values of the module named after it. Rename \
                             either.",
                            decl.label, inner.label
                        ));
                    }
                }
            }
            Kind::Struct | Kind::Union => {}
        }
    }
}

/// Whether the codegen can declare `name`: an ASCII identifier that is not
/// a reserved word in TypeScript, or `__proto__`.
fn is_ts_identifier(name: &str) -> bool {
    const RESERVED: &[&str] = &[
        "arguments",
        "any",
        "await",
        "bigint",
        "boolean",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "eval",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "implements",
        "import",
        "in",
        "infer",
        "instanceof",
        "interface",
        "keyof",
        "let",
        "new",
        "never",
        "null",
        "number",
        "object",
        "package",
        "private",
        "protected",
        "public",
        "readonly",
        "return",
        "static",
        "string",
        "super",
        "switch",
        "symbol",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "undefined",
        "unique",
        "unknown",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ];
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
        && !RESERVED.contains(&name)
        && name != "__proto__"
}
