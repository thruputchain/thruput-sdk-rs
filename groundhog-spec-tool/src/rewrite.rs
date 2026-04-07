use core::ops::Range;

use wasm_encoder::{CustomSection, Module, RawSection};
use wasmparser::Parser;
use xdrs::cxs::{decode, encode};

use crate::{
    ContractSpec, SpecFragment, SpecToolError, canonicalize_spec,
    raw::{FINAL_SECTION_NAME, RAW_SECTION_NAME, parse_raw_fragments},
};

pub fn rewrite_wasm(
    bytes: &[u8],
    package_name: &str,
) -> Result<Vec<u8>, SpecToolError> {
    Ok(rewrite_wasm_with_spec(bytes, package_name)?.wasm)
}

pub struct RewrittenWasm {
    pub wasm: Vec<u8>,
    pub spec: Option<ContractSpec>,
}

pub fn rewrite_wasm_with_spec(
    bytes: &[u8],
    package_name: &str,
) -> Result<RewrittenWasm, SpecToolError> {
    let raw_fragments = collect_raw_fragments(bytes)?;
    let spec = if raw_fragments.is_empty() {
        collect_final_spec(bytes)?
    } else {
        match canonicalize_spec(package_name, &raw_fragments) {
            Ok(spec) => Some(spec),
            Err(SpecToolError::NoSpecFragments) => None,
            Err(err) => return Err(err),
        }
    };
    ensure_raw_fragments_stay_out_of_linear_memory(&raw_fragments, bytes)?;
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

    if let Some(spec) = &spec {
        module.section(&CustomSection {
            name: FINAL_SECTION_NAME.into(),
            data: encode(spec).expect("contract specs must XDR encode").into(),
        });
    }
    Ok(RewrittenWasm {
        wasm: module.finish(),
        spec,
    })
}

fn collect_raw_fragments(
    bytes: &[u8],
) -> Result<Vec<SpecFragment>, SpecToolError> {
    let mut raw_fragments = Vec::<SpecFragment>::new();

    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::CustomSection(section) = payload {
            if section.name() != RAW_SECTION_NAME {
                continue;
            }

            raw_fragments.extend(parse_raw_fragments(section.data())?);
        }
    }

    Ok(raw_fragments)
}

fn collect_final_spec(
    bytes: &[u8],
) -> Result<Option<ContractSpec>, SpecToolError> {
    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::CustomSection(section) = payload {
            if section.name() != FINAL_SECTION_NAME {
                continue;
            }

            return decode(section.data())
                .map(Some)
                .map_err(|err| SpecToolError::Wasm(err.to_string()));
        }
    }

    Ok(None)
}

fn ensure_raw_fragments_stay_out_of_linear_memory(
    raw_fragments: &[SpecFragment],
    bytes: &[u8],
) -> Result<(), SpecToolError> {
    if raw_fragments.is_empty() {
        return Ok(());
    }
    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::DataSection(reader) = payload {
            for segment in reader {
                let segment = segment
                    .map_err(|err| SpecToolError::Wasm(err.to_string()))?;
                for fragment in raw_fragments {
                    let encoded = encode(fragment).expect(
                        "raw spec fragments must XDR encode for leak checks",
                    );
                    if contains_subslice(segment.data, &encoded) {
                        return Err(SpecToolError::Wasm(
                            "raw spec bytes leaked into linear memory"
                                .to_string(),
                        ));
                    }
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
