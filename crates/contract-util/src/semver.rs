/// Parse a version string and return the major version
///
/// Accepts `MAJOR.MINOR.PATCH` with optional `-PRERELEASE` and `+BUILD`
/// suffixes, with the same numeric-field strictness as the `semver` crate
/// (digits only, no leading zeros). Panics on malformed input; use
/// [`try_major_version_from_string`] to validate first.
pub fn major_version_from_string(version_str: &str) -> String {
    try_major_version_from_string(version_str)
        .unwrap_or_else(|| panic!("invalid semver string: {version_str:?}"))
}

/// Fallible form of [`major_version_from_string`]: returns `None` instead
/// of panicking on malformed input.
pub fn try_major_version_from_string(version_str: &str) -> Option<String> {
    let (major, minor, patch) = parse_triplet(version_str)?;
    Some(major_version(version_str, major, minor, patch))
}

/// The major version is the leftmost non-zero component:
/// - `"1.2.3"` → `"1"` (major is non-zero)
/// - `"0.9.2"` → `"0.9"` (major is 0, minor is non-zero)
/// - `"0.0.2"` → `"0.0.2"` (0.0.z is always considered major)
///
/// For `0.0.0` the full version string (including any pre-release/build
/// suffix) is returned. The result feeds selector hashes, so it must stay
/// byte-identical to what `semver::Version::to_string()` produced: for
/// inputs the strict parser accepts, that is the input string itself.
fn major_version(
    version_str: &str,
    major: u64,
    minor: u64,
    patch: u64,
) -> String {
    if major > 0 {
        major.to_string()
    } else if minor > 0 {
        format!("{major}.{minor}")
    } else if patch > 0 {
        format!("{major}.{minor}.{patch}")
    } else {
        version_str.to_string()
    }
}

fn parse_triplet(version_str: &str) -> Option<(u64, u64, u64)> {
    // Build metadata (`+...`) comes after any pre-release (`-...`), so strip
    // it first; pre-release identifiers may themselves contain hyphens.
    let core = version_str.split_once('+').map_or(version_str, |(v, _)| v);
    let core = core.split_once('-').map_or(core, |(v, _)| v);

    let mut fields = core.split('.');
    let major = parse_numeric_field(fields.next()?)?;
    let minor = parse_numeric_field(fields.next()?)?;
    let patch = parse_numeric_field(fields.next()?)?;
    if fields.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn parse_numeric_field(field: &str) -> Option<u64> {
    if field.is_empty()
        || (field.len() > 1 && field.starts_with('0'))
        || !field.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    field.parse().ok()
}
