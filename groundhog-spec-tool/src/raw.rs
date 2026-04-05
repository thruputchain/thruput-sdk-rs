use crate::{RawInterfaceFragment, SpecToolError};

pub const RAW_SECTION_NAME: &str = "groundhogspecrawv0";
pub const FINAL_SECTION_NAME: &str = "groundhogspecv0";
pub const RAW_RECORD_TAG: &str = "iface";
pub const FINAL_FORMAT_TAG: &str = "gh_iface_v1";

pub fn encode_raw_fragment(fragment: &RawInterfaceFragment) -> Vec<u8> {
    format!(
        "{RAW_RECORD_TAG}\t{}\t{}\t{}\t{}\n",
        fragment.package_name,
        fragment.self_type_key.as_deref().unwrap_or(""),
        fragment.interface_name,
        fragment.interface_version,
    )
    .into_bytes()
}

pub fn parse_raw_fragment_line(line: &str) -> Result<RawInterfaceFragment, SpecToolError> {
    let parts = line.split('\t').collect::<Vec<_>>();
    match parts.as_slice() {
        [RAW_RECORD_TAG, package_name, self_type_key, interface_name, interface_version] => {
            Ok(RawInterfaceFragment {
                package_name: (*package_name).to_owned(),
                self_type_key: if self_type_key.is_empty() {
                    None
                } else {
                    Some((*self_type_key).to_owned())
                },
                interface_name: (*interface_name).to_owned(),
                interface_version: (*interface_version).to_owned(),
            })
        }
        _ => Err(SpecToolError::InvalidRawFragment(line.to_owned())),
    }
}
