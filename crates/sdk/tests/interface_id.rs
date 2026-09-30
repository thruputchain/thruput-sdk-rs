//! `#[contract]` derives one interface ID per (interface name, major
//! version): the trait's associated const, the `<Iface>Entrypoints`
//! wrapper's const, and `groundhog_contract_util::interface_id` must agree,
//! and the ID must move only with the major version.

#![allow(unexpected_cfgs)]

use groundhog_contract_util::interface_id;
use sdk::*;

#[contract(version = "1.2.3")]
pub trait Counter {
    fn get(&self) -> u32;
}

#[contract(version = "2.0.0")]
pub trait CounterV2 {
    fn get(&self) -> u32;
}

/// Same interface name and major line as `Counter`, later patch version.
/// Its own module, because the generated client and wrapper names derive
/// from the interface name.
#[allow(dead_code)]
mod patch {
    use sdk::*;

    #[contract(version = "1.9.0", interface = "Counter")]
    pub trait CounterPatch {
        fn get(&self) -> u32;
    }
}

#[test]
fn trait_wrapper_and_util_agree() {
    assert_eq!(
        <CounterContract as Counter>::INTERFACE_ID,
        CounterEntrypoints::INTERFACE_ID
    );
    assert_eq!(
        CounterEntrypoints::INTERFACE_ID,
        interface_id(Some("Counter"), "1.2.3")
    );
}

#[test]
fn id_follows_name_and_major_version_only() {
    // A patch release of the same interface line keeps the ID.
    assert_eq!(
        patch::CounterEntrypoints::INTERFACE_ID,
        CounterEntrypoints::INTERFACE_ID
    );
    // A major bump changes it, and so does the name.
    assert_ne!(
        CounterV2Entrypoints::INTERFACE_ID,
        CounterEntrypoints::INTERFACE_ID
    );
    assert_ne!(
        interface_id(Some("Counter"), "1.2.3"),
        interface_id(Some("CounterV2"), "1.2.3")
    );
}

/// The host interface table holds the impl blocks of the contract crate
/// that expanded them, whichever struct carries them: a contract is an
/// image, not a storage struct, and `implements!` scopes to the invoking
/// package.
#[test]
fn interface_table_reflects_the_contract_crate() {
    use sdk::interface_table::implements_in;

    assert!(sdk::implements!(CounterEntrypoints::INTERFACE_ID));
    assert!(!sdk::implements!(CounterV2Entrypoints::INTERFACE_ID));
    // The same query from another package, or another version of this one,
    // sees a different contract.
    assert!(!implements_in(
        "some-other-contract",
        env!("CARGO_PKG_VERSION"),
        CounterEntrypoints::INTERFACE_ID
    ));
    assert!(!implements_in(
        env!("CARGO_PKG_NAME"),
        "0.0.0-other",
        CounterEntrypoints::INTERFACE_ID
    ));
}

#[sdk::storage]
struct CounterContract {
    value: sdk::storage::Value<u32>,
}

#[contract]
impl Counter for CounterContract {
    fn get(&self) -> u32 {
        self.value.try_get().none_if_missing().unwrap_or(0)
    }
}
