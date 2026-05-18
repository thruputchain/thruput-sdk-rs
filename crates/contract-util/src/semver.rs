use semver::Version;

/// Parse a version string and return the major version
pub fn major_version_from_string(version_str: &str) -> String {
    let version = Version::parse(version_str).unwrap();
    major_version(&version)
}

/// The major version is the leftmost non-zero component:
/// - `"1.2.3"` → `"1"` (major is non-zero)
/// - `"0.9.2"` → `"0.9"` (major is 0, minor is non-zero)
/// - `"0.0.2"` → `"0.0.2"` (0.0.z is always considered major)
pub fn major_version(version: &Version) -> String {
    if version.major > 0 {
        version.major.to_string()
    } else if version.minor > 0 {
        format!("{}.{}", version.major, version.minor)
    } else if version.patch > 0 {
        format!("{}.{}.{}", version.major, version.minor, version.patch)
    } else {
        version.to_string()
    }
}
