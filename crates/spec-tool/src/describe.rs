use groundhog_contract_spec::{
    ContractExceptionType, ContractType, ContractTypeDef, SpecFragmentView,
};
use std::io::{self, Write};
use xdrs::txs;

pub struct DescribableType {
    pub name: &'static str,
    /// `CARGO_PKG_NAME` of the crate defining the type — the selector
    /// preimage's crate component.
    pub crate_name: &'static str,
    /// Major version in the selector preimage (`"1"`, `"0.1"`, ...), as
    /// derived by `groundhog_contract_util::semver` from the version the
    /// type's macro baked in. The registry invariant test recomputes each
    /// selector from these fields, so an entry that skews from its crate's
    /// actual selector fails the build here.
    pub major_version: &'static str,
    pub selector: u64,
    pub fragments: &'static [SpecFragmentView<'static>],
    pub root: &'static str,
}

const LIBRARY_EXN_FRAGMENTS: &[SpecFragmentView<'static>] = &[
    sdk::exn::__groundhog_spec_fragment_LibraryExn,
    sdk::exn::__groundhog_spec_fragment_SyscallExn,
    sdk::exn::__groundhog_spec_fragment_CallingError,
    sdk::exn::__groundhog_spec_fragment_HandlingError,
    sdk::exn::__groundhog_spec_fragment_AuthTokenError,
    sdk::exn::__groundhog_spec_fragment_FaultString,
];

const ERC20_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] =
    &[groundhog_erc20_if::__groundhog_spec_fragment_ERC20Error];

const ERC1271_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] =
    &[groundhog_erc1271_if::__groundhog_spec_fragment_ERC1271Error];

const ERC20_PAUSABLE_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] = &[
    groundhog_erc20_pausable_if::__groundhog_spec_fragment_ERC20PausableError,
];

const ACCESS_CONTROL_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] = &[
    groundhog_access_control_if::__groundhog_spec_fragment_AccessControlError,
];

const SIGNED_OP_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] =
    &[groundhog_signed_op_if::__groundhog_spec_fragment_SignedOpError];

pub const REGISTRY: &[DescribableType] = &[
    DescribableType {
        name: "LibraryExn",
        crate_name: "sdk",
        // The sdk pins `#[exn(version = "1.0.0")]` rather than tracking
        // CARGO_PKG_VERSION.
        major_version: "1",
        selector: <sdk::exn::LibraryExn as sdk::exn::Exn>::SELECTOR,
        fragments: LIBRARY_EXN_FRAGMENTS,
        root: "LibraryExn",
    },
    DescribableType {
        name: "ERC20Error",
        crate_name: "groundhog-erc20-if",
        major_version: "0.1",
        selector: <groundhog_erc20_if::ERC20Error as sdk::exn::Exn>::SELECTOR,
        fragments: ERC20_ERROR_FRAGMENTS,
        root: "ERC20Error",
    },
    DescribableType {
        name: "ERC1271Error",
        crate_name: "groundhog-erc1271-if",
        major_version: "0.1",
        selector:
            <groundhog_erc1271_if::ERC1271Error as sdk::exn::Exn>::SELECTOR,
        fragments: ERC1271_ERROR_FRAGMENTS,
        root: "ERC1271Error",
    },
    DescribableType {
        name: "ERC20PausableError",
        crate_name: "groundhog-erc20-pausable-if",
        major_version: "0.1",
        selector: <groundhog_erc20_pausable_if::ERC20PausableError
            as sdk::exn::Exn>::SELECTOR,
        fragments: ERC20_PAUSABLE_ERROR_FRAGMENTS,
        root: "ERC20PausableError",
    },
    DescribableType {
        name: "AccessControlError",
        crate_name: "groundhog-access-control-if",
        major_version: "0.1",
        selector: <groundhog_access_control_if::AccessControlError
            as sdk::exn::Exn>::SELECTOR,
        fragments: ACCESS_CONTROL_ERROR_FRAGMENTS,
        root: "AccessControlError",
    },
    DescribableType {
        name: "SignedOpError",
        crate_name: "groundhog-signed-op-if",
        major_version: "0.1",
        selector:
            <groundhog_signed_op_if::SignedOpError as sdk::exn::Exn>::SELECTOR,
        fragments: SIGNED_OP_ERROR_FRAGMENTS,
        root: "SignedOpError",
    },
];

pub fn lookup(name: &str) -> Option<&'static DescribableType> {
    REGISTRY.iter().find(|t| t.name == name)
}

pub fn lookup_by_selector(selector: u64) -> Option<&'static DescribableType> {
    REGISTRY.iter().find(|t| t.selector == selector)
}

pub struct DescribeOutput {
    pub exception: ContractExceptionType,
    pub type_defs: Vec<ContractTypeDef>,
}

pub fn build_describe(ty: &DescribableType) -> DescribeOutput {
    let type_defs: Vec<ContractTypeDef> = ty
        .fragments
        .iter()
        .filter_map(|frag| match frag {
            SpecFragmentView::TypeDef(def)
            | SpecFragmentView::ExportedTypeDef(def) => {
                Some(ContractTypeDef::from(*def))
            }
            _ => None,
        })
        .collect();

    DescribeOutput {
        exception: ContractExceptionType {
            selector: ty.selector,
            ty: ContractType::Named {
                named: ty.root.as_bytes().try_into().expect(
                    "registered describe root must fit in ContractSpecString",
                ),
            },
        },
        type_defs,
    }
}

pub enum OutputFormat {
    Json,
    Xdr,
}

/// Write a describe result to `sink`.
///
/// The `Xdr` format is a bare concatenation with no framing:
/// `encode(exception) ++ encode(type_defs)`. It is not self-describing — a
/// consumer must decode a `ContractExceptionType` first and then a
/// `Vec<ContractTypeDef>` from the remaining bytes.
pub fn write_output<W: Write>(
    output: &DescribeOutput,
    format: OutputFormat,
    sink: &mut W,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "exception": {
                    "selector": format!("0x{:016x}", output.exception.selector),
                    "type": crate::type_to_json(&output.exception.ty),
                },
                "type_defs": output.type_defs.iter().map(crate::type_def_to_json).collect::<Vec<_>>(),
            });
            sink.write_all(
                serde_json::to_string(&json)
                    .expect("JSON serialization must succeed")
                    .as_bytes(),
            )?;
            sink.write_all(b"\n")?;
        }
        OutputFormat::Xdr => {
            let mut bytes = txs::encode(&output.exception).map_err(|e| {
                io::Error::other(format!("xdr encode exception: {e}"))
            })?;
            let defs_bytes = txs::encode(&output.type_defs).map_err(|e| {
                io::Error::other(format!("xdr encode type_defs: {e}"))
            })?;
            bytes.extend_from_slice(&defs_bytes);
            sink.write_all(&bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn selectors_match_recorded_preimage() {
        for entry in REGISTRY {
            let expected =
                groundhog_contract_util::exception_selector_from_major(
                    entry.major_version,
                    entry.crate_name,
                    entry.name,
                );
            assert_eq!(
                entry.selector,
                expected,
                "registry entry {} has selector 0x{:016x} but its recorded \
                 preimage ({} :: {} :: {}) hashes to 0x{:016x}",
                entry.name,
                entry.selector,
                entry.major_version,
                entry.crate_name,
                entry.name,
                expected,
            );
        }
    }

    #[test]
    fn names_and_selectors_unique() {
        for (i, a) in REGISTRY.iter().enumerate() {
            for b in &REGISTRY[i + 1..] {
                assert_ne!(a.name, b.name);
                assert_ne!(
                    a.selector, b.selector,
                    "{} and {} share a selector",
                    a.name, b.name
                );
            }
        }
    }

    #[test]
    fn lookup_by_selector_finds_each_entry() {
        for entry in REGISTRY {
            let found = lookup_by_selector(entry.selector)
                .expect("registered selector must resolve");
            assert_eq!(found.name, entry.name);
        }
        assert!(lookup_by_selector(0).is_none());
    }
}
