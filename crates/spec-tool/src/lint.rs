//! Findings that do not make a spec invalid, but that a known consumer of it
//! will reject.
//!
//! Canonicalization accepts these specs, and the contract deploys and runs;
//! what fails is a later tool reading the spec. Reporting them when the spec
//! is built puts the diagnostic where the name can still be changed cheaply.

use crate::{
    ContractSpec, ContractSpecId, ContractTypeDef, spec_id_lossy,
    spec_str_lossy,
};

/// The global object generated TypeScript reaches built-in types through
/// (`globalThis.Uint8Array`). A spec name spelled this way would shadow it.
const TS_GLOBAL_OBJECT: &str = "globalThis";

/// The prefix `groundhog codegen ts` gives the aliases it declares when a
/// spec name shadows one of its own.
const TS_ESCAPE_PREFIX: &str = "__gh_";

/// A spec finding a consumer of the spec will reject.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpecWarning {
    /// A name in the spec lands on an identifier `groundhog codegen ts`
    /// reserves, so TypeScript bindings cannot be generated for the spec.
    ///
    /// `segment` is the offending crate, module, type, event, or interface
    /// name; `item` names the declaration it belongs to.
    ReservedTypeScriptName { item: String, segment: String },
}

impl core::fmt::Display for SpecWarning {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ReservedTypeScriptName { item, segment } => {
                let reserved_for = if segment == TS_GLOBAL_OBJECT {
                    "the global object it reaches built-in types through"
                } else {
                    "the aliases it declares, which start with \"__gh_\""
                };
                write!(
                    f,
                    "{item} has \"{segment}\" in its name, which \
                     `groundhog codegen ts` reserves for {reserved_for}; \
                     it will refuse to generate TypeScript bindings for \
                     this spec. Rename it."
                )
            }
        }
    }
}

/// Report the names in `spec` that a consumer of it will reject.
///
/// Every declared type and event nests in generated TypeScript under its
/// crate and module path, and every interface is declared at the top level,
/// so each of those names is checked.
pub fn spec_warnings(spec: &ContractSpec) -> Vec<SpecWarning> {
    let mut warnings = Vec::new();
    for def in spec.type_defs.iter() {
        let (kind, id) = match def {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                ("struct", &struct_def.name)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                ("enum", &enum_def.name)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                ("union", &union_def.name)
            }
            _ => continue,
        };
        check_id(kind, id, &mut warnings);
    }
    for event in spec.events.iter() {
        check_id("event", &event.name, &mut warnings);
    }
    for interface in spec.interfaces.iter() {
        let name = spec_str_lossy(&interface.name);
        if let Some(segment) = reserved_ts_name(&name) {
            warnings.push(SpecWarning::ReservedTypeScriptName {
                item: format!("interface `{name}`"),
                segment: segment.to_owned(),
            });
        }
    }
    warnings
}

/// Check each segment of `id` that lands in a generated TypeScript path.
///
/// Cargo package names are hyphenated where TypeScript takes `_`, so the
/// crate segment is compared in that form.
fn check_id(kind: &str, id: &ContractSpecId, out: &mut Vec<SpecWarning>) {
    let krate = spec_str_lossy(&id.crate_name).replace('-', "_");
    let module_path = spec_str_lossy(&id.module_path);
    let declared = spec_str_lossy(&id.declared);
    let segments = std::iter::once(krate.as_str())
        .chain(module_path.split("::"))
        .chain(std::iter::once(declared.as_ref()));
    if let Some(segment) = segments.filter_map(reserved_ts_name).next() {
        out.push(SpecWarning::ReservedTypeScriptName {
            item: format!("{kind} `{}`", spec_id_lossy(id)),
            segment: segment.to_owned(),
        });
    }
}

fn reserved_ts_name(name: &str) -> Option<&str> {
    (name == TS_GLOBAL_OBJECT || name.starts_with(TS_ESCAPE_PREFIX))
        .then_some(name)
}
