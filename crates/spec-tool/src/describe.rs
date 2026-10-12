use std::io::{self, Write};
use thruput_contract_spec::{
    ContractExceptionType, ContractSpecIdView, ContractType, ContractTypeDef,
    ContractTypeName, SpecFragmentView,
};
use xdrs::txs;

pub struct DescribableType {
    pub name: &'static str,
    pub selector: u64,
    pub fragments: &'static [SpecFragmentView<'static>],
    /// The id of the root type among `fragments`, taken from the type's own
    /// [`ContractTypeName`] rather than spelled out, so it is whatever that
    /// type calls itself and cannot skew from the def in `fragments`.
    ///
    /// The spec identity is distinct from the wire preimage when an exception
    /// explicitly pins `wire_crate`, as the native token precompile requires.
    pub root: ContractSpecIdView<'static>,
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
    &[thruput_erc20_if::__groundhog_spec_fragment_ERC20Error];

const ERC20_PAUSABLE_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] = &[
    thruput_erc20_pausable_if::__groundhog_spec_fragment_ERC20PausableError,
];

const ACCESS_CONTROL_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] = &[
    thruput_access_control_if::__groundhog_spec_fragment_AccessControlError,
];

const SIGNED_OP_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] =
    &[thruput_signed_op_if::__groundhog_spec_fragment_SignedOpError];

const ERC20_PERMIT_ERROR_FRAGMENTS: &[SpecFragmentView<'static>] =
    &[thruput_erc20_permit_if::__groundhog_spec_fragment_ERC20PermitError];

pub const REGISTRY: &[DescribableType] = &[
    DescribableType {
        name: "LibraryExn",
        selector: <sdk::exn::LibraryExn as sdk::exn::Exn>::SELECTOR,
        fragments: LIBRARY_EXN_FRAGMENTS,
        root: <sdk::exn::LibraryExn as ContractTypeName>::NAME,
    },
    DescribableType {
        name: "ERC20Error",
        selector: <thruput_erc20_if::ERC20Error as sdk::exn::Exn>::SELECTOR,
        fragments: ERC20_ERROR_FRAGMENTS,
        root: <thruput_erc20_if::ERC20Error as ContractTypeName>::NAME,
    },
    DescribableType {
        name: "ERC20PausableError",
        selector: <thruput_erc20_pausable_if::ERC20PausableError
            as sdk::exn::Exn>::SELECTOR,
        fragments: ERC20_PAUSABLE_ERROR_FRAGMENTS,
        root: <thruput_erc20_pausable_if::ERC20PausableError as ContractTypeName>::NAME,
    },
    DescribableType {
        name: "AccessControlError",
        selector: <thruput_access_control_if::AccessControlError
            as sdk::exn::Exn>::SELECTOR,
        fragments: ACCESS_CONTROL_ERROR_FRAGMENTS,
        root: <thruput_access_control_if::AccessControlError as ContractTypeName>::NAME,
    },
    DescribableType {
        name: "SignedOpError",
        selector:
            <thruput_signed_op_if::SignedOpError as sdk::exn::Exn>::SELECTOR,
        fragments: SIGNED_OP_ERROR_FRAGMENTS,
        root: <thruput_signed_op_if::SignedOpError as ContractTypeName>::NAME,
    },
    DescribableType {
        name: "ERC20PermitError",
        selector:
            <thruput_erc20_permit_if::ERC20PermitError as sdk::exn::Exn>::SELECTOR,
        fragments: ERC20_PERMIT_ERROR_FRAGMENTS,
        root: <thruput_erc20_permit_if::ERC20PermitError as ContractTypeName>::NAME,
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
                named: ty.root.into(),
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
    fn native_token_selectors_match_the_protocol_constants() {
        assert_eq!(lookup("ERC20Error").unwrap().selector, 0xb19947acb2fa14ff);
        assert_eq!(
            lookup("ERC20PermitError").unwrap().selector,
            0xe8abb072f5171148
        );
    }

    #[test]
    fn selectors_match_recorded_preimage() {
        for entry in REGISTRY {
            // These protocol-defined selectors retain their pre-rename package
            // labels; all other entries use their declaring package's identity.
            let wire_crate = match entry.name {
                "ERC20Error" => "groundhog-erc20-if",
                "ERC20PermitError" => "groundhog-erc20-permit-if",
                _ => entry.root.crate_name,
            };
            let expected =
                thruput_contract_util::exception_selector_from_major(
                    entry.root.major,
                    wire_crate,
                    entry.root.declared,
                );
            assert_eq!(
                entry.selector,
                expected,
                "registry entry {} has selector 0x{:016x} but its recorded \
                 preimage ({} :: {} :: {}) hashes to 0x{:016x}",
                entry.name,
                entry.selector,
                entry.root.major,
                wire_crate,
                entry.root.declared,
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
