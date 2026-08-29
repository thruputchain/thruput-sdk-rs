//! Negative duplicate-entrypoint fixture.
//!
//! Two modules each declare and implement their own `trait Ledger` with a
//! method named `clash`.  Entrypoint IDs hash only (interface name, major
//! version, method name) — not the self type or module path — so both impls
//! produce the SAME ID.  Guard symbols carrying the ID make this a
//! duplicate-symbol error on EVERY build:
//!
//! - The host `cargo build`/`cargo test` fails at codegen on the
//!   crate-scoped guards (`…::clash__crate__<hex-name>__<hex-version>`) — the loop a
//!   contract author (or their AI assistant) actually iterates in, which
//!   previously compiled duplicates silently while the mock dispatch
//!   let the last registration win.
//! - The rv32 guest build additionally carries unscoped guards, replacing
//!   the original silent behavior where two dispatch-table records shared
//!   an ID and the runtime's first-match-wins scan routed every call to
//!   whichever impl the linker placed first.

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
