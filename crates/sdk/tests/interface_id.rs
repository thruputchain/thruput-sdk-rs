//! `#[contract]` derives one interface ID per (interface name, major
//! version): the trait's associated const, the `<Iface>Entrypoints`
//! wrapper's const, the client's `ContractClient::INTERFACE_ID` and
//! `thruput_contract_util::interface_id` must agree, and the ID must move
//! only with the major version.

#![allow(unexpected_cfgs)]

use thruput_contract_util::interface_id;
use thruput_sdk::*;

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
    use thruput_sdk::*;

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
    assert_eq!(
        CounterClient::INTERFACE_ID,
        CounterEntrypoints::INTERFACE_ID
    );
    // An `interface = ...` rename names the client's interface too.
    assert_eq!(
        patch::CounterClient::INTERFACE_ID,
        patch::CounterEntrypoints::INTERFACE_ID
    );
    assert_eq!(
        BareContractClient::INTERFACE_ID,
        BareContractEntrypoints::INTERFACE_ID
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
    use thruput_sdk::interface_table::implements_in;

    assert!(thruput_sdk::implements!(CounterEntrypoints::INTERFACE_ID));
    assert!(!thruput_sdk::implements!(
        CounterV2Entrypoints::INTERFACE_ID
    ));
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

#[thruput_sdk::storage]
struct CounterContract {
    value: thruput_sdk::storage::Value<u32>,
}

#[contract]
impl Counter for CounterContract {
    fn get(&self) -> u32 {
        self.value.try_get().none_if_missing().unwrap_or(0)
    }
}

/// A non-trait impl block: its client and wrapper share the impl's
/// interface ID too.
#[thruput_sdk::storage]
struct BareContract;

#[contract]
impl BareContract {
    pub fn ping(&self) -> u32 {
        1
    }
}
