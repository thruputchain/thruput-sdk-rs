//! Trait identity survives package versions, Rust aliases and re-exports.

#![allow(unexpected_cfgs)]

use sdk::*;
use version_mismatch_helper::VersionedTrait as Imported;

pub struct ExternalImpl;

#[contract]
impl Imported for ExternalImpl {
    fn versioned_method() -> u32 {
        42
    }
}

mod interfaces {
    use sdk::*;

    #[contract(version = "7.3.2-rc.1+build", interface = "WireCounter")]
    pub trait RustCounter {
        fn increment(value: u32) -> u32;
    }
}

mod reexports {
    pub use super::interfaces::RustCounter as CounterAlias;
}

pub struct LocalImpl;

#[contract]
impl reexports::CounterAlias for LocalImpl {
    fn increment(value: u32) -> u32 {
        value + 1
    }
}

#[test]
fn inherited_ids_match_clients_and_interface_table() {
    assert_eq!(
        <ExternalImpl as Imported>::__GROUNDHOG_ENTRYPOINT_versioned_method,
        version_mismatch_helper::VersionedTraitEntrypoints::versioned_method
            .into_u64()
    );
    assert_eq!(
        <LocalImpl as interfaces::RustCounter>::__GROUNDHOG_ENTRYPOINT_increment,
        interfaces::WireCounterEntrypoints::increment.into_u64()
    );
    assert_eq!(
        <LocalImpl as interfaces::RustCounter>::__GROUNDHOG_IFACE_FULL_VERSION,
        interfaces::WireCounterClient::INTERFACE_VERSION
    );
    assert!(sdk::implements!(
        version_mismatch_helper::VersionedTraitEntrypoints::INTERFACE_ID
    ));
    assert!(sdk::implements!(
        interfaces::WireCounterEntrypoints::INTERFACE_ID
    ));
    assert!(!sdk::implements!(groundhog_contract_util::interface_id(
        Some("CounterAlias"),
        env!("CARGO_PKG_VERSION")
    )));
}

#[test]
fn clients_call_mocks_registered_with_inherited_ids() {
    const ADDRESS: ContractAddress = ContractAddress::new([0x41; 32]);
    const CALLER: ContractAddress = ContractAddress::new([0x42; 32]);
    let env = TestEnvironment::no_cost();
    let mock = env.register_mock(|contract| {
        <ExternalImpl as Imported>::__mock(contract);
        <LocalImpl as interfaces::RustCounter>::__mock(contract);
    });
    mock.deploy_contract_to(&ADDRESS);
    env.transact(|tx| {
        tx.call(&CALLER).run(|| {
            let external =
                version_mismatch_helper::VersionedClient::new(&ADDRESS);
            assert_eq!(external.versioned_method(), 42);
            let local = interfaces::WireCounterClient::new(&ADDRESS);
            assert_eq!(local.increment(41), 42);
        })
    })
    .assert_success();
}
