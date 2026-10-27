//! docs.rs builds without network access, so the build script takes a
//! checked-in copy of the prebuilt archive's `bindings.rs` there (see
//! `DOCS_RS` in `build.rs`). These tests keep that copy in step with the
//! bindings the archive ships.
//!
//! To refresh it after bumping `TEST_FFI_VERSION`, copy `bindings.rs` from
//! the new linux-x86_64 archive to `docs-rs/bindings.rs`.

const CHECKED_IN: &str = include_str!("../docs-rs/bindings.rs");

/// Byte-for-byte check, possible only when this build downloaded the archive
/// the checked-in copy came from. Bindings from other archives and from
/// source builds differ in the libc constants bindgen picks up from the host.
#[cfg(thruput_sys_docs_rs_archive)]
#[test]
fn checked_in_bindings_match_prebuilt_archive() {
    let downloaded = include_str!(concat!(env!("OUT_DIR"), "/bindings.rs"));
    assert!(
        CHECKED_IN == downloaded,
        "docs-rs/bindings.rs differs from the bindings.rs in the \
         linux-x86_64 archive; replace it with the archive's copy"
    );
}

/// Version check for every build method and platform.
#[test]
fn checked_in_bindings_match_ffi_version() {
    let expected = [
        ("MAJOR", thruput_sys::TEST_FFI_VERSION_MAJOR),
        ("MINOR", thruput_sys::TEST_FFI_VERSION_MINOR),
        ("PATCH", thruput_sys::TEST_FFI_VERSION_PATCH),
    ];
    for (part, value) in expected {
        let line =
            format!("pub const TEST_FFI_VERSION_{part}: u32 = {value};");
        assert!(
            CHECKED_IN.lines().any(|l| l == line),
            "docs-rs/bindings.rs is not for this TEST_FFI_VERSION (missing \
             `{line}`); replace it with the linux-x86_64 archive's copy"
        );
    }
}
