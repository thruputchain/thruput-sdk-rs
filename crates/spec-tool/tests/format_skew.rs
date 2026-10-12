//! Raw spec wire-format skew, in both directions.
//!
//! The SDK compiled into a contract and the `thruput-spec-tool` linked into
//! the `thruput` CLI are released independently, so either side can be the
//! stale one. Neither direction may surface as a fragment parse error: the
//! raw section is an unframed XDR stream, so a layout disagreement
//! desynchronizes the reader and produces a message that points nowhere near
//! the actual cause. That is the failure these tests exist to prevent — it
//! shipped once, when `emits` was added to `SpecInterfaceFragment` without a
//! version bump.
//!
//! The pipeline these guard is: read a section name, [`classify_section`] it,
//! and — for a `Raw` section — run [`check_raw_spec_format`] *before* handing
//! the body to [`parse_raw_fragments`].

use thruput_spec_tool::{
    RAW_SECTION_PREFIX, RAW_SPEC_FORMAT, SpecSection, SpecToolError,
    check_raw_spec_format, classify_section, parse_raw_fragments,
};

/// The name a contract built at `format` puts on its raw spec section.
fn raw_section_name(format: u32) -> String {
    format!("{RAW_SECTION_PREFIX}v{format}")
}

/// Deliberately not a valid fragment stream: the version check must fire
/// *before* decoding, since a desynchronized decode is exactly the confusing
/// failure being ruled out. A test that used well-formed bytes could pass
/// while the check ran too late to matter.
const NOT_A_FRAGMENT_STREAM: &[u8] = b"not a valid fragment stream";

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
fn a_newer_raw_spec_section_is_rejected_as_skew() {
    let format = RAW_SPEC_FORMAT + 1;
    assert_eq!(
        classify_section(&raw_section_name(format)),
        SpecSection::Raw(format),
    );
    assert_eq!(
        check_raw_spec_format(format),
        Err(SpecToolError::RawSpecFormatTooNew {
            found: format,
            supported: RAW_SPEC_FORMAT,
        }),
    );
}

#[test]
fn an_older_raw_spec_section_is_rejected_as_skew() {
    let format = RAW_SPEC_FORMAT - 1;
    assert_eq!(
        classify_section(&raw_section_name(format)),
        SpecSection::Raw(format),
    );
    assert_eq!(
        check_raw_spec_format(format),
        Err(SpecToolError::RawSpecFormatTooOld {
            found: format,
            supported: RAW_SPEC_FORMAT,
        }),
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
    assert!(too_new.contains("Upgrade the Thruput CLI"), "{too_new}");

    let too_old = SpecToolError::RawSpecFormatTooOld {
        found: 2,
        supported: 5,
    }
    .to_string();
    assert!(too_old.contains('2') && too_old.contains('5'), "{too_old}");
    assert!(too_old.contains("older"), "{too_old}");
    assert!(too_old.contains("Rebuild the contract"), "{too_old}");
}

/// A skewed section must be diagnosed as skew, not as a missing spec and not
/// as a corrupt fragment stream. Matching the section name exactly (rather
/// than by prefix) would make a skewed artifact look like a contract with no
/// spec at all — the wrong diagnosis, and the one the CLI gave before this
/// check existed. Decoding first would report the desynchronized parse.
#[test]
fn skewed_section_is_not_mistaken_for_a_missing_or_corrupt_spec() {
    for format in [RAW_SPEC_FORMAT - 1, RAW_SPEC_FORMAT + 1] {
        let SpecSection::Raw(found) =
            classify_section(&raw_section_name(format))
        else {
            panic!("format {format} was not classified as a raw spec section");
        };

        let err = check_raw_spec_format(found)
            .expect_err("skewed section must be an error");
        assert!(
            matches!(
                err,
                SpecToolError::RawSpecFormatTooNew { .. }
                    | SpecToolError::RawSpecFormatTooOld { .. }
            ),
            "format {format} was not reported as version skew: {err}",
        );

        // What the caller would have gotten had it decoded first: an
        // unrelated-looking parse failure. This is the diagnosis the version
        // check exists to pre-empt, so the two must stay distinguishable.
        assert!(matches!(
            parse_raw_fragments(NOT_A_FRAGMENT_STREAM),
            Err(SpecToolError::InvalidRawFragment(_)),
        ));
    }
}

/// Sections that merely resemble the raw section name are not spec data, so
/// the prefix match cannot swallow unrelated tooling sections.
#[test]
fn unrelated_sections_are_untouched() {
    // No version suffix.
    assert_eq!(classify_section(RAW_SECTION_PREFIX), SpecSection::Other);
    // Not a number where the version belongs.
    assert_eq!(
        classify_section(&format!("{RAW_SECTION_PREFIX}vnope")),
        SpecSection::Other,
    );
    assert_eq!(classify_section("producers"), SpecSection::Other);
    assert_eq!(classify_section(".text"), SpecSection::Other);
}
