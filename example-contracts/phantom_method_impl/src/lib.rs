//! Phantom-method shaking example contract.
//!
//! Implements `phantom_method_if_a::Ledger` (method `ping`) while linking
//! `phantom_method_if_b`, whose unrelated `Ledger` collides on the interface
//! key with identical metadata but a different method set (`steal`).  The
//! module `local_collider` adds a third colliding candidate from THIS crate
//! (same trait ident, same crate version) that is never implemented.
//!
//! This contract is perfectly valid and must build.  Its published spec must
//! advertise exactly the methods with real entrypoints: `ping` — never
//! `steal`, `phantom_local`, or `steal`'s `LootConfig` payload type.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use phantom_method_if_b::{LedgerClient as LedgerClientB, LootConfig};
use sdk::*;

pub struct PhantomMethodContract;

/// Implements A's `Ledger` and forwards to a B ledger through its client —
/// the linkage that drags B's colliding candidate into the artifact.
#[contract]
impl phantom_method_if_a::Ledger for PhantomMethodContract {
    fn ping() -> u32 {
        LedgerClientB::new(&ContractAddress::new([0u8; 32]))
            .steal(LootConfig { amount: 1 })
    }
}

/// Same-crate collider: an unimplemented trait with the same ident (and
/// therefore the same interface key) as the implemented one.  Its
/// `phantom_local` candidate must likewise be shaken from the spec.
pub mod local_collider {
    use sdk::*;

    #[contract]
    pub trait Ledger {
        fn phantom_local() -> u32;
    }
}
