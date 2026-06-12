use core::ops::Range;

use wasm_encoder::{CustomSection, Module, RawSection};
use wasmparser::Parser;
use xdrs::txs::{decode, encode};

use crate::{
    ContractSpec, SpecFragment, SpecToolError, canonicalize_spec,
    raw::{FINAL_SECTION_NAME, RAW_SECTION_NAME, parse_raw_fragments},
};

/// Rewrite a wasm binary's contract-spec custom sections, returning the new
/// binary. See [`rewrite_wasm_with_spec`] for the exact semantics.
pub fn rewrite_wasm(bytes: &[u8]) -> Result<Vec<u8>, SpecToolError> {
    Ok(rewrite_wasm_with_spec(bytes)?.wasm)
}

/// Result of [`rewrite_wasm_with_spec`].
pub struct RewrittenWasm {
    /// The rewritten wasm binary, with raw spec sections stripped and a
    /// single final spec section appended (when a spec was found).
    pub wasm: Vec<u8>,
    /// The canonicalized spec embedded in `wasm`, or `None` when the input
    /// contained no spec sections at all.
    pub spec: Option<ContractSpec>,
}

/// Canonicalize a wasm binary's contract-spec sections.
///
/// Semantics:
/// - All `groundhogspecrawv0` (raw fragment) and `groundhogspecv0` (final)
///   custom sections are stripped, and a single final section containing the
///   canonicalized spec is re-emitted.
/// - If the input has no raw fragments but does carry a final section, that
///   spec is decoded and re-encoded (idempotent re-runs).
/// - If the input has no spec sections at all, the output carries none and
///   `spec` is `None`.
/// - Raw fragments that define types but no interface occur for raw
///   contracts (hand-written `export_name` entrypoints, no `#[contract]`)
///   that link SDK code carrying typedefs — e.g. `test_bytestring`,
///   `calldata_raw`. The fragments are stripped and `spec` is `None`,
///   **provided the module exports at least one function**. If it exports
///   none, this is a hard error: an entrypoint-less contract cannot be
///   invoked and is almost certainly missing its `#[contract]` macro (this
///   caught exactly that bug in `genesis_deployer`).
/// - Errors with "raw spec bytes leaked into linear memory" if any raw
///   fragment's encoding appears in a data segment — raw fragments must live
///   only in custom sections, never in the contract's linear memory image.
pub fn rewrite_wasm_with_spec(
    bytes: &[u8],
) -> Result<RewrittenWasm, SpecToolError> {
    let raw_fragments = collect_raw_fragments(bytes)?;
    let spec = if raw_fragments.is_empty() {
        collect_final_spec(bytes)?
    } else {
        match canonicalize_spec(&raw_fragments) {
            Ok(spec) => Some(spec),
            Err(SpecToolError::NoSpecFragments) => {
                // Typedefs but no interface: benign for raw contracts,
                // fatal for a contract that forgot #[contract] — the
                // distinguishing signal is whether anything is exported.
                ensure_module_has_function_exports(bytes)?;
                None
            }
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

fn ensure_module_has_function_exports(
    bytes: &[u8],
) -> Result<(), SpecToolError> {
    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::ExportSection(reader) = payload {
            for export in reader {
                let export = export
                    .map_err(|err| SpecToolError::Wasm(err.to_string()))?;
                if export.kind == wasmparser::ExternalKind::Func {
                    return Ok(());
                }
            }
        }
    }
    Err(SpecToolError::InvalidRawFragment(
        "raw spec sections contain type definitions but no interface, and \
         the module exports no functions: the contract is likely missing \
         its #[contract] macro (a contract with no entrypoints cannot be \
         invoked)"
            .to_string(),
    ))
}

fn ensure_raw_fragments_stay_out_of_linear_memory(
    raw_fragments: &[SpecFragment],
    bytes: &[u8],
) -> Result<(), SpecToolError> {
    if raw_fragments.is_empty() {
        return Ok(());
    }
    let encoded_fragments: Vec<Vec<u8>> = raw_fragments
        .iter()
        .map(|fragment| {
            encode(fragment)
                .expect("raw spec fragments must XDR encode for leak checks")
        })
        .collect();
    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|err| SpecToolError::Wasm(err.to_string()))?;

        if let wasmparser::Payload::DataSection(reader) = payload {
            for segment in reader {
                let segment = segment
                    .map_err(|err| SpecToolError::Wasm(err.to_string()))?;
                for encoded in &encoded_fragments {
                    if contains_subslice(segment.data, encoded) {
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
