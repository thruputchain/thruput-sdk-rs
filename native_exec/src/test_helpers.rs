use groundhog_xdr::types::ContractAddress;
use sha2::{Digest, Sha256};

/// Create deterministic test addresses from human-readable names.
///
/// ```
/// use groundhog_native_exec::test_helpers::Named;
/// use groundhog_xdr::types::ContractAddress;
///
/// let alice = ContractAddress::named("alice");
/// assert_eq!(alice, ContractAddress::named("alice")); // deterministic
/// assert_ne!(alice, ContractAddress::named("bob"));   // distinct
/// ```
pub trait Named {
    fn named(name: &str) -> Self;
}

impl Named for ContractAddress {
    fn named(name: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"groundhog-test-address:");
        hasher.update(name.as_bytes());
        let result: [u8; 32] = hasher.finalize().into();
        result.into()
    }
}
