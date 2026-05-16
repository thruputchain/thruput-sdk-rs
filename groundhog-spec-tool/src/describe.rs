use crate::{SpecToolError, canonicalize_type_defs};
use groundhog_contract_spec::{
    ContractExceptionType, ContractSpecString, ContractType, ContractTypeDef,
    SpecFragment, SpecFragmentView,
};
use std::io::{self, Write};
use xdrs::txs;

/// A named SDK-shared exception type that `describe` can emit.
pub struct DescribableType {
    pub name: &'static str,
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
    sdk::storage::error::__groundhog_spec_fragment_StorageError,
];

pub const REGISTRY: &[DescribableType] = &[DescribableType {
    name: "LibraryExn",
    selector: <sdk::exn::LibraryExn as sdk::exn::Exn>::SELECTOR,
    fragments: LIBRARY_EXN_FRAGMENTS,
    root: "LibraryExn",
}];

pub fn lookup(name: &str) -> Option<&'static DescribableType> {
    REGISTRY.iter().find(|t| t.name == name)
}

pub struct DescribeOutput {
    pub exception: ContractExceptionType,
    pub type_defs: Vec<ContractTypeDef>,
}

pub fn build_describe(
    ty: &DescribableType,
) -> Result<DescribeOutput, SpecToolError> {
    let fragments: Vec<SpecFragment> = ty
        .fragments
        .iter()
        .map(|view| SpecFragment::from(*view))
        .collect();
    let root: ContractSpecString = ty
        .root
        .as_bytes()
        .try_into()
        .expect("registered describe root must fit in ContractSpecString");
    let spec = canonicalize_type_defs(&fragments, &[root.clone()])?;

    let exception = ContractExceptionType {
        selector: ty.selector,
        ty: ContractType::Named { named: root },
    };

    Ok(DescribeOutput {
        exception,
        type_defs: spec.type_defs.into(),
    })
}

pub enum OutputFormat {
    Json,
    Xdr,
}

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
