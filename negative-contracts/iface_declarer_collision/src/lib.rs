//! Negative spec fixture: two interfaces the wire cannot tell apart.
//!
//! An interface is keyed by (name, version), and that key is all the wire
//! carries: `interface_id` and every entrypoint id hash the bare trait ident
//! and major version, so that independent crates can implement one interface.
//! The two `Token` traits below therefore share the key `Token@<version>` and
//! one interface id, while their methods differ, so their entrypoints link.
//!
//! Before interface fragments named their declarer, spec-tool could not tell
//! the two apart and published one `Token` interface holding both `ping` and
//! `pong`. Now each fragment carries the trait's spec id, and an artifact
//! anchoring two declarers under one key is rejected
//! (`ConflictingInterfaceDeclarers`). The same holds across crates; one crate
//! with two modules is simply the smallest way to spell it.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

pub mod ledger {
    use sdk::*;

    #[contract]
    pub trait Token {
        fn ping() -> u32;
    }
}

pub mod vault {
    use sdk::*;

    /// Same ident as `ledger::Token`, so the same interface key and id; a
    /// different method, so a different entrypoint and a clean link.
    #[contract]
    pub trait Token {
        fn pong() -> u32;
    }
}

pub struct TwoTokens;

mod ledger_impl {
    use sdk::*;

    #[contract]
    impl crate::ledger::Token for crate::TwoTokens {
        fn ping() -> u32 {
            1
        }
    }
}

mod vault_impl {
    use sdk::*;

    #[contract]
    impl crate::vault::Token for crate::TwoTokens {
        fn pong() -> u32 {
            2
        }
    }
}
