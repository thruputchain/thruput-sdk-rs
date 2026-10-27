//! Wire-format versioning for the raw spec ELF section.
//!
//! The raw section body is an unframed concatenation of XDR-encoded
//! [`SpecFragment`](crate::SpecFragment) values, so a parser that disagrees
//! with the producer about the fragment layout does not fail cleanly: it
//! reads the field set it knows, stops short, and resumes decoding at a byte
//! offset inside the previous fragment. Everything after that decodes as
//! garbage, and nothing in the stream distinguishes "newer than me" from
//! "corrupt".
//!
//! The section *name* carries the version so that disagreement is detectable
//! before a single fragment byte is read. Producers emit
//! [`RAW_SECTION_NAME`]; consumers match on [`RAW_SECTION_PREFIX`] and
//! compare the suffix against [`RAW_SPEC_FORMAT`], which lets an unreadable
//! artifact be reported as a version skew naming both sides rather than as a
//! parse failure or a missing spec.
//!
//! # Bumping
//!
//! Any change to the encoding of `SpecFragment` or of a type reachable from
//! it is a format change and must bump [`RAW_SPEC_FORMAT`]. Four mirrors
//! must move together, and each is pinned by a test so a missed one fails CI
//! rather than shipping:
//!
//! - this constant,
//! - `[package.metadata.thruput] raw_spec_format` in this crate's
//!   `Cargo.toml`, which is how the `thruput` CLI reads the format version
//!   out of a contract's dependency graph before compiling it,
//! - the `link_section` literals in `thruput-macros`,
//! - the `(INFO)` output section in `thruput-rv32-runtime`'s
//!   `thruput-rv32.ld`, which keeps the section out of guest memory.

/// Prefix shared by every version of the raw spec ELF section name.
///
/// Consumers match on this rather than on [`RAW_SECTION_NAME`] so that a
/// section written by a different SDK is recognized as a raw spec section of
/// the wrong version, instead of being skipped as unrelated (which surfaces
/// as "this contract has no spec" — the wrong diagnosis).
pub const RAW_SECTION_PREFIX: &str = "thruputspecraw";

/// Wire-format version of the raw spec section this SDK produces and parses.
pub const RAW_SPEC_FORMAT: u32 = 5;

/// Name of the raw spec ELF section this SDK emits.
///
/// Kept as a literal rather than concatenated from [`RAW_SECTION_PREFIX`] and
/// [`RAW_SPEC_FORMAT`] because `#[link_section = ...]` in `thruput-macros`
/// needs a literal anyway; `raw_section_name_matches_its_parts` pins the
/// three against each other.
pub const RAW_SECTION_NAME: &str = "thruputspecrawv5";

/// Name of the finalized spec ELF section.
///
/// It holds a `ContractSpec`, whose layout is governed by the protocol-level
/// `thruput-spec` crate, so the suffix moves when that layout does. `v2`
/// adds method effects; `v1` added segmented spec ids. Rebuild contracts and
/// bindings after updating the toolchain; legacy decoding is not supported.
///
/// Unlike the raw section, the suffix is not parsed: [`classify_section`]
/// matches this name exactly, so a `v0` section reads as
/// [`SpecSection::Other`] and the artifact reports as having no spec rather
/// than as a version skew. That is the wrong diagnosis, but it is a safe one
/// -- the alternative, decoding `v0` bytes as `v1`, misaligns silently. A
/// finalized section only ever reaches a reader in an unlowered build
/// artifact: `thruput-rv32-lower` drops both spec sections, so nothing
/// deployed carries one.
pub const FINAL_SECTION_NAME: &str = "thruputspecv2";

/// What an ELF section name means to the spec pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecSection {
    /// A raw fragment section carrying the given wire-format version. The
    /// version may differ from [`RAW_SPEC_FORMAT`]; callers must check.
    Raw(u32),
    /// The finalized spec section.
    Final,
    /// Not a spec section.
    Other,
}

/// Classify an ELF section name.
///
/// A raw section whose suffix is not `v<integer>` classifies as
/// [`SpecSection::Other`]: an unparseable suffix cannot be reported as a
/// version skew, and treating it as a raw section would only turn it into a
/// decode failure later.
pub fn classify_section(name: &str) -> SpecSection {
    if name == FINAL_SECTION_NAME {
        return SpecSection::Final;
    }
    match name
        .strip_prefix(RAW_SECTION_PREFIX)
        .and_then(|suffix| suffix.strip_prefix('v'))
        .and_then(|digits| digits.parse::<u32>().ok())
    {
        Some(version) => SpecSection::Raw(version),
        None => SpecSection::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_section_name_matches_its_parts() {
        assert_eq!(
            classify_section(RAW_SECTION_NAME),
            SpecSection::Raw(RAW_SPEC_FORMAT),
            "RAW_SECTION_NAME must be RAW_SECTION_PREFIX + 'v' + \
             RAW_SPEC_FORMAT, or producers and consumers disagree",
        );
    }

    #[test]
    fn rv32_linker_script_keeps_raw_spec_out_of_guest_memory() {
        // Without its own non-alloc output section the raw spec is an orphan
        // ALLOC section that lld places in DATA, where it counts against the
        // guest memory size the heap ASSERT checks. Whole trimmed lines are
        // compared, so a typo on either side of the name fails, and nothing
        // allocates, so the test builds without the `alloc` feature.
        let script = include_str!("../../rv32-runtime/link/thruput-rv32.ld");
        let has_line = |prefix: &str, suffix: &str| {
            script.lines().any(|line| {
                line.trim()
                    .strip_prefix(prefix)
                    .and_then(|rest| rest.strip_suffix(suffix))
                    == Some(RAW_SECTION_NAME)
            })
        };
        assert!(
            has_line("", " 0 (INFO) :") && has_line("KEEP(*(", "))"),
            "thruput-rv32.ld must place {RAW_SECTION_NAME} in an (INFO) \
             output section; update it alongside RAW_SPEC_FORMAT",
        );
    }

    #[test]
    fn manifest_metadata_matches_raw_spec_format() {
        // The CLI reads this key out of a contract's resolved dependency
        // graph via `cargo metadata` to detect skew before compiling, so it
        // must track the constant. See the module docs on bumping.
        let manifest = include_str!("../Cargo.toml");
        let declared = manifest
            .lines()
            .find_map(|line| line.trim().strip_prefix("raw_spec_format"))
            .and_then(|rest| rest.trim().strip_prefix('='))
            .and_then(|rest| rest.trim().parse::<u32>().ok())
            .expect(
                "Cargo.toml must carry `[package.metadata.thruput] \
                 raw_spec_format = <n>` so `cargo metadata` reports the \
                 format version this crate emits",
            );
        assert_eq!(
            declared, RAW_SPEC_FORMAT,
            "Cargo.toml's raw_spec_format must match RAW_SPEC_FORMAT",
        );
    }

    #[test]
    fn classifies_section_names() {
        assert_eq!(
            classify_section(RAW_SECTION_NAME),
            SpecSection::Raw(RAW_SPEC_FORMAT),
        );
        assert_eq!(
            classify_section("thruputspecrawv99"),
            SpecSection::Raw(99),
        );
        assert_eq!(classify_section(FINAL_SECTION_NAME), SpecSection::Final);
        assert_eq!(classify_section("name"), SpecSection::Other);
        // Unparseable suffixes are not raw sections: there is no version to
        // report, so a decode failure is no better than ignoring them.
        for name in [
            "thruputspecraw",
            "thruputspecrawvx",
            "thruputspecrawv1x",
            "thruputspecrawv-1",
        ] {
            assert_eq!(classify_section(name), SpecSection::Other, "{name}");
        }
    }
}
