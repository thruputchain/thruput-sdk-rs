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

    /// major_version_from_string should be idempotent: applying it to its
    /// own output (re-parsed as a version) should yield the same string.
    /// This matters because the error selector and entrypoint ID both
    /// feed the result into a hash — if the function weren't stable,
    /// the same version could produce different selectors.
    #[test]
    fn test_major_version_idempotent() {
        let cases = [
            "1.2.3", "2.0.0", "10.5.99", "0.9.2", "0.1.0", "0.15.3", "0.0.2",
            "0.0.1", "0.0.0",
        ];
        for version_str in cases {
            let first = major_version_from_string(version_str);
            // Pad the major version back to a parseable semver string
            let reparseable = match first.matches('.').count() {
                0 => format!("{}.0.0", first),
                1 => format!("{}.0", first),
                _ => first.clone(),
            };
            let second = major_version_from_string(&reparseable);
            assert_eq!(
                first, second,
                "not idempotent for input {}: first={}, reparsed as {}, second={}",
                version_str, first, reparseable, second
            );
        }
    }

    #[test]
    fn test_error_selector_deterministic() {
        let s1 = crate::error_selector("1.2.3", "mycrate", "MyError");
        let s2 = crate::error_selector("1.2.3", "mycrate", "MyError");
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_error_selector_differs_by_version() {
        let s1 = crate::error_selector("1.0.0", "mycrate", "MyError");
        let s2 = crate::error_selector("2.0.0", "mycrate", "MyError");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_error_selector_differs_by_name() {
        let s1 = crate::error_selector("1.0.0", "mycrate", "AlphaError");
        let s2 = crate::error_selector("1.0.0", "mycrate", "BetaError");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_error_selector_same_major_same_selector() {
        // 1.2.3 and 1.9.0 have the same major version "1"
        let s1 = crate::error_selector("1.2.3", "mycrate", "MyError");
        let s2 = crate::error_selector("1.9.0", "mycrate", "MyError");
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_error_selector_differs_by_crate() {
        let s1 = crate::error_selector("1.0.0", "crate_a", "MyError");
        let s2 = crate::error_selector("1.0.0", "crate_b", "MyError");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_event_signature_deterministic() {
        let s1 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
        let s2 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_event_signature_is_32_bytes() {
        let sig = crate::event_signature("1.0.0", "mycrate", "MyEvent");
        assert_eq!(sig.len(), 32);
    }

    #[test]
    fn test_event_signature_same_major_same_signature() {
        let s1 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
        let s2 = crate::event_signature("1.9.0", "mycrate", "MyEvent");
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_event_signature_differs_by_version() {
        let s1 = crate::event_signature("1.0.0", "mycrate", "MyEvent");
        let s2 = crate::event_signature("2.0.0", "mycrate", "MyEvent");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_event_signature_differs_by_crate() {
        let s1 = crate::event_signature("1.0.0", "crate_a", "MyEvent");
        let s2 = crate::event_signature("1.0.0", "crate_b", "MyEvent");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_event_signature_differs_by_name() {
        let s1 = crate::event_signature("1.0.0", "mycrate", "AlphaEvent");
        let s2 = crate::event_signature("1.0.0", "mycrate", "BetaEvent");
        assert_ne!(s1, s2);
    }
}
