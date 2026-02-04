mod tests {
    use crate::semver::major_version_from_string;

    #[test]
    fn test_cargo_major_version_major_nonzero() {
        assert_eq!(major_version_from_string("1.2.3"), "1");
        assert_eq!(major_version_from_string("2.0.0"), "2");
        assert_eq!(major_version_from_string("10.5.99"), "10");
    }

    #[test]
    fn test_cargo_major_version_minor_nonzero() {
        assert_eq!(major_version_from_string("0.9.2"), "0.9");
        assert_eq!(major_version_from_string("0.1.0"), "0.1");
        assert_eq!(major_version_from_string("0.15.3"), "0.15");
    }

    #[test]
    fn test_cargo_major_version_patch_only() {
        assert_eq!(major_version_from_string("0.0.2"), "0.0.2");
        assert_eq!(major_version_from_string("0.0.1"), "0.0.1");
        assert_eq!(major_version_from_string("0.0.0"), "0.0.0");
    }

    #[test]
    fn test_cargo_major_version_edge_cases() {
        // Single component
        assert_eq!(major_version_from_string("1.0.0-alpha.1"), "1");
        assert_eq!(
            major_version_from_string("0.0.1-alpha.1+build123"),
            "0.0.1"
        );
        assert_eq!(major_version_from_string("0.0.0-beta.1"), "0.0.0-beta.1");
    }
}
