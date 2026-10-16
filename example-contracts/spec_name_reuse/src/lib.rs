//! Every collision qualified spec ids resolve, in one contract that builds.
//!
//! None of this was possible before user-defined types and events were named
//! by `CRATE@MAJOR::module::path::Name`. Each pair below shares a bare ident,
//! which is all the spec used to record, so each pair was either merged into
//! one entry or rejected outright as a `ConflictingTypeDef` naming a type the
//! author never wrote — and nothing in this contract could be changed to
//! avoid it, because the names belong to its dependencies.
//!
//! 1. Two exceptions called `Error`, one from each interface crate, both
//!    reachable through `throws`. The motivating case: an `Error` per library
//!    is the most ordinary thing a Rust API does.
//! 2. Two events called `Updated`, one from each interface crate, both
//!    reachable through `emits`. Their ids differ, and so do their on-wire
//!    signatures, which are hashed per declaring crate.
//! 3. Two types called `Config` in two modules of THIS crate, with different
//!    shapes, both reachable from methods. Same ident, one crate — the case a
//!    single author can hit alone.
//!
//! dev.sh asserts the built spec carries all six ids distinctly.
//!
//! Both `Config`s deliberately differ in shape: identical shapes would have
//! merged silently under the old naming, which is the quieter half of the bug
//! but not the half that stopped a build.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;

/// Reuses `vault::Config`'s ident inside one crate.
pub mod ledger {
    use thruput_sdk::*;

    #[contract_type]
    pub struct Config {
        pub limit: u32,
    }
}

/// Reuses `ledger::Config`'s ident, with a different shape.
pub mod vault {
    use thruput_sdk::*;

    #[contract_type]
    pub struct Config {
        pub ceiling: u64,
        pub frozen: bool,
    }
}

pub struct NameReuse;

/// Both `Config` types reach the spec from here.
#[contract]
impl NameReuse {
    pub fn set_ledger(cfg: ledger::Config) -> u32 {
        cfg.limit
    }

    pub fn set_vault(cfg: vault::Config) -> u64 {
        if cfg.frozen { 0 } else { cfg.ceiling }
    }
}

/// Brings `dup_name_if_a`'s `Error` and `Updated` into the spec.
#[contract]
impl dup_name_if_a::StoreA for NameReuse {
    fn set_limit(limit: u32) -> u32 {
        limit
    }
}

/// Brings `dup_name_if_b`'s identically-named `Error` and `Updated` in beside
/// them.
#[contract]
impl dup_name_if_b::StoreB for NameReuse {
    fn set_ceiling(ceiling: u64) -> u64 {
        ceiling
    }
}
