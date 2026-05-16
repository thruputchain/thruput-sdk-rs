use crate::{
    canonicalize_type_defs, render_spec_json, SpecToolError,
};
use groundhog_contract_spec::{
    ContractSpec, ContractSpecString, SpecFragment, SpecFragmentView,
};
use std::io::{self, Write};
use xdrs::txs;

/// A named SDK-shared type that `describe` can emit.
pub struct DescribableType {
    pub name: &'static str,
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
    fragments: LIBRARY_EXN_FRAGMENTS,
    root: "LibraryExn",
}];

pub fn lookup(name: &str) -> Option<&'static DescribableType> {
    REGISTRY.iter().find(|t| t.name == name)
}

pub fn build_spec(ty: &DescribableType) -> Result<ContractSpec, SpecToolError> {
    let fragments: Vec<SpecFragment> = ty
        .fragments
        .iter()
        .map(|view| SpecFragment::from(*view))
        .collect();
    let root: ContractSpecString = ty.root.as_bytes().try_into().unwrap();
    canonicalize_type_defs(&fragments, &[root])
}

pub enum OutputFormat {
    Json,
    Xdr,
}

pub fn write_output<W: Write>(
    spec: &ContractSpec,
    format: OutputFormat,
    sink: &mut W,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => {
            sink.write_all(render_spec_json(spec).as_bytes())?;
            sink.write_all(b"\n")?;
        }
        OutputFormat::Xdr => {
            let bytes = txs::encode(spec)
                .map_err(|e| io::Error::other(format!("xdr encode: {e}")))?;
            sink.write_all(&bytes)?;
        }
    }
    Ok(())
}
