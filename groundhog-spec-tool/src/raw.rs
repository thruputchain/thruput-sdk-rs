use groundhog_contract_spec::SpecFragment;
use xdrs::{
    ReadXDR,
    txs::{buf_reader::BufReader, encode},
};

use crate::{RawInterfaceFragment, SpecToolError};

pub const RAW_SECTION_NAME: &str = "groundhogspecrawv0";
pub const FINAL_SECTION_NAME: &str = "groundhogspecv0";
pub const RAW_RECORD_TAG: &str = "xdr_spec_fragment_v1";
pub const FINAL_FORMAT_TAG: &str = "gh_contract_spec_v1";

pub fn encode_raw_fragment(fragment: &RawInterfaceFragment) -> Vec<u8> {
    encode(&SpecFragment::Interface(fragment.clone().into()))
        .expect("spec interface fragments must XDR encode")
}

pub fn parse_raw_fragments(
    bytes: &[u8],
) -> Result<Vec<SpecFragment>, SpecToolError> {
    let mut fragments = Vec::new();
    let mut reader = BufReader::new(bytes);

    while !reader.remaining_slice().is_empty() {
        let before = reader.position();
        let fragment =
            SpecFragment::read_xdr(&mut reader).map_err(|err| {
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

pub fn parse_raw_fragment_line(
    line: &str,
) -> Result<RawInterfaceFragment, SpecToolError> {
    match parse_raw_fragments(line.as_bytes())?.as_slice() {
        [SpecFragment::Interface(fragment)] => Ok(fragment.clone().into()),
        _ => Err(SpecToolError::InvalidRawFragment(
            "expected exactly one interface fragment".to_string(),
        )),
    }
}
