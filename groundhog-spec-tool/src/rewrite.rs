use core::ops::Range;

use wasm_encoder::{CustomSection, Module, RawSection};
use wasmparser::Parser;

use crate::{
    canonicalize_interfaces,
    raw::{parse_raw_fragment_line, FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME},
    RawInterfaceFragment, SpecToolError,
};

pub fn rewrite_wasm(bytes: &[u8]) -> Result<Vec<u8>, SpecToolError> {
    Ok(rewrite_wasm_with_interfaces(bytes)?.wasm)
}

pub struct RewrittenWasm {
    pub wasm: Vec<u8>,
    pub interfaces: Vec<crate::InterfaceId>,
}

pub fn rewrite_wasm_with_interfaces(bytes: &[u8]) -> Result<RewrittenWasm, SpecToolError> {
    let mut raw_fragments = Vec::<RawInterfaceFragment>::new();
    let mut module = Module::new();

    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::CustomSection(section) = &payload {
            match section.name() {
                RAW_SECTION_NAME => {
                    let text = core::str::from_utf8(section.data())
                        .map_err(|err| SpecToolError::Wasm(err.to_string()))?;
                    for line in text.lines().filter(|line| !line.is_empty()) {
                        raw_fragments.push(parse_raw_fragment_line(line)?);
                    }
                    continue;
                }
                FINAL_SECTION_NAME => continue,
                _ => {}
            }
        }

        if let Some((id, range)) = payload.as_section() {
            push_section(&mut module, bytes, id, range);
        }
    }

    let interfaces = canonicalize_interfaces(&raw_fragments)?;
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
    Ok(RewrittenWasm {
        wasm: module.finish(),
        interfaces,
    })
}

fn push_section(module: &mut Module, bytes: &[u8], id: u8, range: Range<usize>) {
    module.section(&RawSection {
        id,
        data: &bytes[range.start..range.end],
    });
}
