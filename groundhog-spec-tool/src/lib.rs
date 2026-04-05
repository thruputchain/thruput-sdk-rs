pub mod model;
pub mod raw;
pub mod rewrite;

pub use model::{InterfaceId, RawInterfaceFragment};
pub use raw::{
    FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_RECORD_TAG, RAW_SECTION_NAME,
    encode_raw_fragment, parse_raw_fragment_line,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecToolError {
    NoInterfaceFragments,
    InvalidRawFragment(String),
    Wasm(String),
}

impl core::fmt::Display for SpecToolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoInterfaceFragments => write!(f, "no interface fragments"),
            Self::InvalidRawFragment(msg) | Self::Wasm(msg) => {
                f.write_str(msg)
            }
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

    let mut interfaces = Vec::with_capacity(raw.len());

    for fragment in raw {
        if fragment.interface_name.is_empty()
            || fragment.interface_version.is_empty()
        {
            return Err(SpecToolError::InvalidRawFragment(
                "interface fragments must include name and version"
                    .to_string(),
            ));
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

pub fn render_spec_json(interfaces: &[InterfaceId]) -> String {
    let mut json = String::from("{\"format\":\"");
    json.push_str(FINAL_FORMAT_TAG);
    json.push_str("\",\"interfaces\":[");

    for (index, interface) in interfaces.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("{\"interface_name\":\"");
        push_json_string(&mut json, &interface.interface_name);
        json.push_str("\",\"interface_version\":\"");
        push_json_string(&mut json, &interface.interface_version);
        json.push_str("\"}");
    }

    json.push_str("]}");
    json
}

fn push_json_string(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                use core::fmt::Write as _;
                write!(out, "\\u{:04x}", ch as u32).unwrap();
            }
            ch => out.push(ch),
        }
    }
}
