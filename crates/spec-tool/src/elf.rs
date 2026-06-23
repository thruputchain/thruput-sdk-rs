//! Extract contract-spec fragments from a riscv32 (or any) ELF's
//! `groundhogspecrawv0` section. The bytes are the same XDR fragment
//! concatenation the wasm path reads from a custom section, so they feed the
//! shared `parse_raw_fragments` + `canonicalize_spec` pipeline unchanged.

use groundhog_contract_spec::ContractSpec;
use object::{Object, ObjectSection};

use crate::{
    canonicalize_spec, parse_raw_fragments, SpecToolError, RAW_SECTION_NAME,
};

/// Concatenate the bytes of every `groundhogspecrawv0` section in `bytes`.
pub fn collect_elf_raw_section(bytes: &[u8]) -> Result<Vec<u8>, SpecToolError> {
    let file = object::File::parse(bytes)
        .map_err(|e| SpecToolError::Elf(e.to_string()))?;
    let mut raw = Vec::new();
    for section in file.sections() {
        let name =
            section.name().map_err(|e| SpecToolError::Elf(e.to_string()))?;
        if name == RAW_SECTION_NAME {
            let data = section
                .data()
                .map_err(|e| SpecToolError::Elf(e.to_string()))?;
            raw.extend_from_slice(data);
        }
    }
    Ok(raw)
}

/// Extract and canonicalize the contract spec from an ELF, or `None` if it
/// carries no spec fragments.
pub fn elf_to_spec(
    bytes: &[u8],
) -> Result<Option<ContractSpec>, SpecToolError> {
    let raw = collect_elf_raw_section(bytes)?;
    if raw.is_empty() {
        return Ok(None);
    }
    let fragments = parse_raw_fragments(&raw)?;
    match canonicalize_spec(&fragments) {
        Ok(spec) => Ok(Some(spec)),
        Err(SpecToolError::NoSpecFragments) => Ok(None),
        Err(err) => Err(err),
    }
}
