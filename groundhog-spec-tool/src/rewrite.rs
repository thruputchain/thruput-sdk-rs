use core::ops::Range;

use wasm_encoder::{CustomSection, Module, RawSection};
use wasmparser::Parser;

use crate::{
    RawInterfaceFragment, SpecToolError, canonicalize_interfaces,
    raw::{
        FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME,
        parse_raw_fragment_line,
    },
};

pub fn rewrite_wasm(
    bytes: &[u8],
    package_name: &str,
) -> Result<Vec<u8>, SpecToolError> {
    Ok(rewrite_wasm_with_interfaces(bytes, package_name)?.wasm)
}

pub struct RewrittenWasm {
    pub wasm: Vec<u8>,
    pub interfaces: Vec<crate::InterfaceId>,
}

pub fn rewrite_wasm_with_interfaces(
    bytes: &[u8],
    package_name: &str,
) -> Result<RewrittenWasm, SpecToolError> {
    let raw_fragments = collect_raw_fragments(bytes)?;
    let owned_raw_fragments = raw_fragments
        .iter()
        .filter(|fragment| fragment.package_name == package_name)
        .cloned()
        .collect::<Vec<_>>();

    let interfaces = if owned_raw_fragments.is_empty() {
        Vec::new()
    } else {
        canonicalize_interfaces(&owned_raw_fragments)?
    };
    let raw_fragment_blob = raw_fragments
        .iter()
        .flat_map(crate::encode_raw_fragment)
        .collect::<Vec<_>>();
    ensure_raw_fragments_stay_out_of_linear_memory(&raw_fragment_blob, bytes)?;
    let mut module = Module::new();

    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        match payload {
            wasmparser::Payload::CustomSection(section) => {
                match section.name() {
                    RAW_SECTION_NAME | FINAL_SECTION_NAME => continue,
                    _ => push_section(&mut module, bytes, 0, section.range()),
                }
            }
            _ => {
                if let Some((id, range)) = payload.as_section() {
                    push_section(&mut module, bytes, id, range);
                }
            }
        }
    }

    if !interfaces.is_empty() {
        let mut final_text = String::from(FINAL_FORMAT_TAG);
        final_text.push('\n');
        for interface in &interfaces {
            final_text.push_str(&interface.interface_name);
            final_text.push('\t');
            final_text.push_str(&interface.interface_version);
            final_text.push('\n');
        }

        module.section(&CustomSection {
            name: FINAL_SECTION_NAME.into(),
            data: final_text.into_bytes().into(),
        });
    }
    Ok(RewrittenWasm {
        wasm: module.finish(),
        interfaces,
    })
}

fn collect_raw_fragments(
    bytes: &[u8],
) -> Result<Vec<RawInterfaceFragment>, SpecToolError> {
    let mut raw_fragments = Vec::<RawInterfaceFragment>::new();

    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::CustomSection(section) = payload {
            if section.name() != RAW_SECTION_NAME {
                continue;
            }

            let text = core::str::from_utf8(section.data())
                .map_err(|err| SpecToolError::Wasm(err.to_string()))?;
            for line in text.lines().filter(|line| !line.is_empty()) {
                raw_fragments.push(parse_raw_fragment_line(line)?);
            }
        }
    }

    Ok(raw_fragments)
}

fn ensure_raw_fragments_stay_out_of_linear_memory(
    raw_fragment_blob: &[u8],
    bytes: &[u8],
) -> Result<(), SpecToolError> {
    if raw_fragment_blob.is_empty() {
        return Ok(());
    }
    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::DataSection(reader) = payload {
            for segment in reader {
                let segment = segment
                    .map_err(|err| SpecToolError::Wasm(err.to_string()))?;
                if contains_subslice(segment.data, raw_fragment_blob) {
                    return Err(SpecToolError::Wasm(
                        "raw interface spec bytes leaked into linear memory"
                            .to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn push_section(
    module: &mut Module,
    bytes: &[u8],
    id: u8,
    range: Range<usize>,
) {
    module.section(&RawSection {
        id,
        data: &bytes[range.start..range.end],
    });
}
