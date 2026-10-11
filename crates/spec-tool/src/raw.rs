use thruput_contract_spec::SpecFragment;
use xdrs::{ReadXDR, txs::buf_reader::BufReader};

use crate::SpecToolError;

/// Section naming and wire-format versioning live in
/// `thruput-contract-spec`, which owns [`SpecFragment`] and so owns what
/// counts as a format change. Re-exported here because consumers (the
/// `thruput` CLI among them) reach them through this crate.
pub use thruput_contract_spec::{
    FINAL_SECTION_NAME, RAW_SECTION_NAME, RAW_SECTION_PREFIX, RAW_SPEC_FORMAT,
    SpecSection, classify_section,
};

pub const FINAL_FORMAT_TAG: &str = "gh_contract_spec_v1";

/// Parse the contents of a raw spec ELF section.
///
/// The section body is a bare concatenation of XDR-encoded [`SpecFragment`]
/// values with no count prefix or framing: fragments are decoded one after
/// another until the input is exhausted. Note that string fields are not
/// UTF-8 validated here; [`crate::canonicalize_spec`] rejects non-UTF-8
/// names.
pub fn parse_raw_fragments(
    bytes: &[u8],
) -> Result<Vec<SpecFragment>, SpecToolError> {
    let mut fragments = Vec::new();
    let mut reader = BufReader::new(bytes);

    while !reader.remaining_slice().is_empty() {
        let before = reader.position();
        let fragment = SpecFragment::read_xdr(&mut reader).map_err(|err| {
            SpecToolError::InvalidRawFragment(err.to_string())
        })?;

        if reader.position() == before {
            return Err(SpecToolError::InvalidRawFragment(
                "spec fragment parser made no progress".to_string(),
            ));
        }

        fragments.push(fragment);
    }

    Ok(fragments)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::RAW_SECTION_NAME;

    #[test]
    fn sdk_macro_link_section_literals_match_raw_section_name() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .expect("spec-tool lives under crates/spec-tool");
        let expected =
            format!(r#"unsafe(link_section = "{RAW_SECTION_NAME}")"#);

        for relative in [
            "crates/macros/src/common/spec_reg.rs",
            "crates/macros/src/derive_contract_type_spec.rs",
            "crates/macros/src/derive_event.rs",
        ] {
            let path = workspace_root.join(relative);
            let source = fs::read_to_string(&path).unwrap_or_else(|err| {
                let path = path.display();
                panic!("failed to read {path}: {err}")
            });
            assert!(
                source.contains(&expected),
                "{} must use the canonical raw spec section name {RAW_SECTION_NAME:?}",
                path.display()
            );
        }
    }
}
