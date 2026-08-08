//! Negative duplicate-entrypoint fixture.
//!
//! Two modules each declare and implement their own `trait Ledger` with a
//! method named `clash`.  Entrypoint IDs hash only (interface name, major
//! version, method name) — not the self type or module path — so both impls
//! produce the SAME ID.  With the wrapper symbol carrying the ID on both
//! guest targets, this must fail the build as a duplicate symbol: on rv32
//! this replaces the previous silent behavior where two dispatch-table
//! records shared an ID and the runtime's first-match-wins scan routed
//! every call to whichever impl the linker placed first.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

pub mod side_a {
    use sdk::*;

    pub struct ContractA;

    #[contract]
    pub trait Ledger {
        fn clash() -> u32;
    }

    #[contract]
    impl Ledger for ContractA {
        fn clash() -> u32 {
            1
        }
    }
}

pub mod side_b {
    use sdk::*;

    pub struct ContractB;

    #[contract]
    pub trait Ledger {
        fn clash() -> u32;
    }

    #[contract]
    impl Ledger for ContractB {
        fn clash() -> u32 {
            2
        }
    }
}
