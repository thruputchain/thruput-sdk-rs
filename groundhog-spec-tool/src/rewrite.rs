use core::ops::Range;

use wasm_encoder::{
    ConstExpr, CustomSection, DataCountSection, DataSection, Module,
    RawSection,
};
use wasmparser::{Data, DataKind, Operator, Parser};

use crate::{
    RawInterfaceFragment, SpecToolError, canonicalize_interfaces,
    raw::{
        FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME,
        parse_raw_fragment_line,
    },
};

pub fn rewrite_wasm(bytes: &[u8]) -> Result<Vec<u8>, SpecToolError> {
    Ok(rewrite_wasm_with_interfaces(bytes)?.wasm)
}

pub struct RewrittenWasm {
    pub wasm: Vec<u8>,
    pub interfaces: Vec<crate::InterfaceId>,
}

pub fn rewrite_wasm_with_interfaces(
    bytes: &[u8],
) -> Result<RewrittenWasm, SpecToolError> {
    let raw_fragments = collect_raw_fragments(bytes)?;

    let interfaces = canonicalize_interfaces(&raw_fragments)?;
    let raw_fragment_blob = raw_fragments
        .iter()
        .flat_map(crate::encode_raw_fragment)
        .collect::<Vec<_>>();
    let filtered_data_segment_count =
        count_kept_data_segments(&raw_fragment_blob, bytes)?;
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
            wasmparser::Payload::DataCountSection { .. } => {
                module.section(&DataCountSection {
                    count: filtered_data_segment_count,
                });
            }
            wasmparser::Payload::DataSection(reader) => {
                push_filtered_data_section(
                    &mut module,
                    bytes,
                    &raw_fragment_blob,
                    reader,
                )?;
            }
            _ => {
                if let Some((id, range)) = payload.as_section() {
                    push_section(&mut module, bytes, id, range);
                }
            }
        }
    }

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

fn count_kept_data_segments(
    raw_fragment_blob: &[u8],
    bytes: &[u8],
) -> Result<u32, SpecToolError> {
    let mut kept = 0;

    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::DataSection(reader) = payload {
            for segment in reader {
                let segment = segment
                    .map_err(|err| SpecToolError::Wasm(err.to_string()))?;
                if !matches!(
                    rewrite_data_segment(&segment, raw_fragment_blob)?,
                    DataSegmentRewrite::Drop
                ) {
                    kept += 1;
                }
            }
        }
    }

    Ok(kept)
}

fn push_filtered_data_section(
    module: &mut Module,
    bytes: &[u8],
    raw_fragment_blob: &[u8],
    reader: wasmparser::DataSectionReader<'_>,
) -> Result<(), SpecToolError> {
    let mut data_section = DataSection::new();

    for segment in reader {
        let segment =
            segment.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        match rewrite_data_segment(&segment, raw_fragment_blob)? {
            DataSegmentRewrite::Drop => {}
            DataSegmentRewrite::KeepRaw => {
                data_section
                    .raw(&bytes[segment.range.start..segment.range.end]);
            }
            DataSegmentRewrite::ReencodeActive {
                memory_index,
                offset,
                data,
            } => {
                let offset = ConstExpr::i32_const(offset);
                data_section.active(
                    memory_index,
                    &offset,
                    data.iter().copied(),
                );
            }
        }
    }

    if !data_section.is_empty() {
        module.section(&data_section);
    }

    Ok(())
}

enum DataSegmentRewrite<'a> {
    Drop,
    KeepRaw,
    ReencodeActive {
        memory_index: u32,
        offset: i32,
        data: &'a [u8],
    },
}

fn rewrite_data_segment<'a>(
    segment: &'a Data<'a>,
    raw_fragment_blob: &[u8],
) -> Result<DataSegmentRewrite<'a>, SpecToolError> {
    if raw_fragment_blob.is_empty() {
        return Ok(DataSegmentRewrite::KeepRaw);
    }

    let DataKind::Active {
        memory_index,
        offset_expr,
    } = &segment.kind
    else {
        return Ok(DataSegmentRewrite::KeepRaw);
    };

    if !segment.data.starts_with(raw_fragment_blob) {
        return Ok(DataSegmentRewrite::KeepRaw);
    }

    let remaining_data = &segment.data[raw_fragment_blob.len()..];
    if remaining_data.is_empty() {
        return Ok(DataSegmentRewrite::Drop);
    }

    let raw_blob_len =
        i32::try_from(raw_fragment_blob.len()).map_err(|_| {
            SpecToolError::Wasm(
                "raw interface fragments exceed i32 length".to_string(),
            )
        })?;
    let rebased_offset = parse_i32_const_offset(offset_expr)?
        .checked_add(raw_blob_len)
        .ok_or_else(|| {
            SpecToolError::Wasm(
                "rebased data segment offset overflowed i32".to_string(),
            )
        })?;

    Ok(DataSegmentRewrite::ReencodeActive {
        memory_index: *memory_index,
        offset: rebased_offset,
        data: remaining_data,
    })
}

fn parse_i32_const_offset(
    offset_expr: &wasmparser::ConstExpr<'_>,
) -> Result<i32, SpecToolError> {
    let mut ops = offset_expr.get_operators_reader();
    let offset = match ops
        .read()
        .map_err(|err| SpecToolError::Wasm(err.to_string()))?
    {
        Operator::I32Const { value } => value,
        other => {
            return Err(SpecToolError::Wasm(format!(
                "unsupported data segment offset expression: {other:?}"
            )));
        }
    };
    match ops
        .read()
        .map_err(|err| SpecToolError::Wasm(err.to_string()))?
    {
        Operator::End => Ok(offset),
        other => Err(SpecToolError::Wasm(format!(
            "unsupported trailing data segment offset operator: {other:?}"
        ))),
    }
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
