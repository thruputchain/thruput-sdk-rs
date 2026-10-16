//! Host-only registry of contract-state root domains, used to detect two
//! roots that share a domain when linked into one artifact. Compiled only on
//! host targets — never present in a guest (rv32) build.

use crate::__linkme::distributed_slice;
use crate::storage::FramedDomain;

#[derive(Clone, Copy, Debug)]
pub struct DomainRecord {
    /// The root's compile-time-framed domain bytes. Byte equality of framed
    /// domains is exactly on-chain root collision (the framing is injective).
    pub domain: FramedDomain,
    pub type_path: &'static str,
}

#[distributed_slice]
pub static STORAGE_DOMAINS: [DomainRecord];

/// Returns the first `(i, j)` pair sharing a domain but naming different
/// types (so the same type registered twice by multiple `#[contract] impl`
/// blocks is not a false positive).
pub fn check_duplicate(records: &[DomainRecord]) -> Option<(usize, usize)> {
    for i in 0..records.len() {
        for j in (i + 1)..records.len() {
            if records[i].domain.as_bytes() == records[j].domain.as_bytes()
                && records[i].type_path != records[j].type_path
            {
                return Some((i, j));
            }
        }
    }
    None
}

/// Panics if two distinct root types share a domain. Called by the
/// `#[contract]`-generated host test.
pub fn assert_unique_storage_domains() {
    let records: &[DomainRecord] = &STORAGE_DOMAINS;
    if let Some((i, j)) = check_duplicate(records) {
        panic!(
            "storage domain collision: {} and {} derive the same storage \
             root; set #[storage(domain = \"...\")] on one of them to \
             disambiguate",
            records[i].type_path, records[j].type_path
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(d: &'static [u8], t: &'static str) -> DomainRecord {
        DomainRecord {
            domain: FramedDomain::new(d),
            type_path: t,
        }
    }

    #[test]
    fn distinct_domains_ok() {
        let r = [rec(b"A", "x::A"), rec(b"B", "y::B")];
        assert_eq!(check_duplicate(&r), None);
    }

    #[test]
    fn same_domain_different_type_flagged() {
        let r = [rec(b"A", "x::A"), rec(b"A", "y::A")];
        assert_eq!(check_duplicate(&r), Some((0, 1)));
    }

    #[test]
    fn same_domain_same_type_is_not_a_collision() {
        // Same type registered twice (two #[contract] impl blocks).
        let r = [rec(b"A", "x::A"), rec(b"A", "x::A")];
        assert_eq!(check_duplicate(&r), None);
    }
}
