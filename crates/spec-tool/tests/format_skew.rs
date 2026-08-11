//! Raw spec wire-format skew, in both directions.
//!
//! The SDK compiled into a contract and the `groundhog-spec-tool` linked into
//! the `groundhog` CLI are released independently, so either side can be the
//! stale one. Neither direction may surface as a fragment parse error: the
//! raw section is an unframed XDR stream, so a layout disagreement
//! desynchronizes the reader and produces a message that points nowhere near
//! the actual cause. That is the failure these tests exist to prevent — it
//! shipped once, when `emits` was added to `SpecInterfaceFragment` without a
//! version bump.

use groundhog_spec_tool::{
    RAW_SECTION_PREFIX, RAW_SPEC_FORMAT, SpecToolError, check_raw_spec_format,
    rewrite::rewrite_wasm,
};
use wasm_encoder::{CustomSection, Module};

/// A wasm module carrying one raw spec section at `format`.
///
/// The body is deliberately not a valid fragment stream: the version check
/// must fire *before* decoding, since a desynchronized decode is exactly the
/// confusing failure being ruled out. A test that used well-formed bytes
/// could pass while the check ran too late to matter.
fn module_with_raw_section(format: u32) -> Vec<u8> {
    let mut module = Module::new();
    module.section(&CustomSection {
        name: format!("{RAW_SECTION_PREFIX}v{format}").as_str().into(),
        data: b"not a valid fragment stream".as_slice().into(),
    });
    module.finish()
}

#[test]
fn check_accepts_only_this_builds_format() {
    assert_eq!(check_raw_spec_format(RAW_SPEC_FORMAT), Ok(()));
    assert_eq!(
        check_raw_spec_format(RAW_SPEC_FORMAT + 1),
        Err(SpecToolError::RawSpecFormatTooNew {
            found: RAW_SPEC_FORMAT + 1,
            supported: RAW_SPEC_FORMAT,
        }),
    );
    assert_eq!(
        check_raw_spec_format(RAW_SPEC_FORMAT - 1),
        Err(SpecToolError::RawSpecFormatTooOld {
            found: RAW_SPEC_FORMAT - 1,
            supported: RAW_SPEC_FORMAT,
        }),
    );
}

#[test]
fn rewrite_rejects_a_newer_raw_spec_format() {
    let err = rewrite_wasm(&module_with_raw_section(RAW_SPEC_FORMAT + 1))
        .expect_err("a newer raw spec format must not be parsed");

    assert_eq!(
        err,
        SpecToolError::RawSpecFormatTooNew {
            found: RAW_SPEC_FORMAT + 1,
            supported: RAW_SPEC_FORMAT,
        },
    );
}

#[test]
fn rewrite_rejects_an_older_raw_spec_format() {
    let err = rewrite_wasm(&module_with_raw_section(RAW_SPEC_FORMAT - 1))
        .expect_err("an older raw spec format must not be parsed");

    assert_eq!(
        err,
        SpecToolError::RawSpecFormatTooOld {
            found: RAW_SPEC_FORMAT - 1,
            supported: RAW_SPEC_FORMAT,
        },
    );
}

/// Both messages must name both versions and say which side to move. A user
/// hitting this has two independently-versioned tools in hand and no way to
/// tell which is stale unless the error says so.
#[test]
fn skew_messages_name_both_versions_and_a_fix() {
    let too_new = SpecToolError::RawSpecFormatTooNew {
        found: 7,
        supported: 3,
    }
    .to_string();
    assert!(too_new.contains('7') && too_new.contains('3'), "{too_new}");
    assert!(too_new.contains("newer"), "{too_new}");
    assert!(too_new.contains("Upgrade the groundhog CLI"), "{too_new}");

    let too_old = SpecToolError::RawSpecFormatTooOld {
        found: 2,
        supported: 5,
    }
    .to_string();
    assert!(too_old.contains('2') && too_old.contains('5'), "{too_old}");
    assert!(too_old.contains("older"), "{too_old}");
    assert!(too_old.contains("Rebuild the contract"), "{too_old}");
}

/// A skewed section must be diagnosed, not silently ignored. Matching the
/// section name exactly (rather than by prefix) would make a skewed artifact
/// look like a contract with no spec at all — the wrong diagnosis, and the
/// one the CLI gave before this check existed.
#[test]
fn skewed_section_is_not_mistaken_for_a_missing_spec() {
    for format in [RAW_SPEC_FORMAT - 1, RAW_SPEC_FORMAT + 1] {
        let err = rewrite_wasm(&module_with_raw_section(format))
            .expect_err("skewed section must be an error");
        assert!(
            !matches!(err, SpecToolError::NoSpecFragments),
            "format {format} reported as a missing spec: {err}",
        );
    }
}

/// Custom sections that merely resemble the raw section name are left alone,
/// so the prefix match cannot swallow unrelated tooling sections.
#[test]
fn unrelated_custom_sections_are_untouched() {
    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_PREFIX.into(), // no version suffix
        data: b"unrelated".as_slice().into(),
    });
    module.section(&CustomSection {
        name: "producers".into(),
        data: b"unrelated".as_slice().into(),
    });
    let bytes = module.finish();

    let rewritten = rewrite_wasm(&bytes)
        .expect("unversioned lookalikes are not spec data");

    let mut names = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&rewritten) {
        if let wasmparser::Payload::CustomSection(section) = payload.unwrap() {
            names.push(section.name().to_owned());
        }
    }
    assert_eq!(
        names,
        vec![RAW_SECTION_PREFIX.to_owned(), "producers".into()]
    );
}
