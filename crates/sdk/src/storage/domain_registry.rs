//! Host-only registry of contract-state root domains, used to detect two
//! roots that share a domain when linked into one artifact. Compiled only on
//! host targets — never present in a guest (wasm/rv32) build.

use crate::__linkme::distributed_slice;

#[derive(Clone, Copy, Debug)]
pub struct DomainRecord {
    pub domain: &'static str,
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
            if records[i].domain == records[j].domain
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
            "storage domain collision: \"{}\" is claimed by both {} and {}; \
             set #[storage(domain = \"...\")] on one of them to disambiguate",
            records[i].domain, records[i].type_path, records[j].type_path
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(d: &'static str, t: &'static str) -> DomainRecord {
        DomainRecord {
            domain: d,
            type_path: t,
        }
    }

    #[test]
    fn distinct_domains_ok() {
        let r = [rec("A", "x::A"), rec("B", "y::B")];
        assert_eq!(check_duplicate(&r), None);
    }

    #[test]
    fn same_domain_different_type_flagged() {
        let r = [rec("A", "x::A"), rec("A", "y::A")];
        assert_eq!(check_duplicate(&r), Some((0, 1)));
    }

    #[test]
    fn same_domain_same_type_is_not_a_collision() {
        // Same type registered twice (two #[contract] impl blocks).
        let r = [rec("A", "x::A"), rec("A", "x::A")];
        assert_eq!(check_duplicate(&r), None);
    }
}
