pub mod model;
pub mod raw;
pub mod rewrite;

pub use model::{InterfaceId, RawInterfaceFragment};
pub use raw::{
    encode_raw_fragment, parse_raw_fragment_line, FINAL_FORMAT_TAG, FINAL_SECTION_NAME,
    RAW_RECORD_TAG, RAW_SECTION_NAME,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecToolError {
    NoInterfaceFragments,
    MultipleImplTypes(String, String),
    InvalidRawFragment(String),
    Wasm(String),
}

impl core::fmt::Display for SpecToolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoInterfaceFragments => write!(f, "no interface fragments"),
            Self::MultipleImplTypes(left, right) => {
                write!(f, "multiple impl-backed self types: {left}, {right}")
            }
            Self::InvalidRawFragment(msg) | Self::Wasm(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for SpecToolError {}

pub fn canonicalize_interfaces(
    raw: &[RawInterfaceFragment],
) -> Result<Vec<InterfaceId>, SpecToolError> {
    if raw.is_empty() {
        return Err(SpecToolError::NoInterfaceFragments);
    }

    let mut impl_self_type: Option<&str> = None;
    let mut interfaces = Vec::with_capacity(raw.len());

    for fragment in raw {
        if fragment.interface_name.is_empty() || fragment.interface_version.is_empty() {
            return Err(SpecToolError::InvalidRawFragment(
                "interface fragments must include name and version".to_string(),
            ));
        }

        if let Some(self_type_key) = fragment.self_type_key.as_deref() {
            match impl_self_type {
                Some(existing) if existing != self_type_key => {
                    return Err(SpecToolError::MultipleImplTypes(
                        existing.to_string(),
                        self_type_key.to_string(),
                    ));
                }
                None => impl_self_type = Some(self_type_key),
                Some(_) => {}
            }
        }

        interfaces.push(InterfaceId::new(
            fragment.interface_name.clone(),
            fragment.interface_version.clone(),
        ));
    }

    interfaces.sort();
    interfaces.dedup();
    Ok(interfaces)
}
