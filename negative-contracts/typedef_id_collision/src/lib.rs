//! Negative spec fixture: two defs claiming one spec id.
//!
//! Qualifying type names stopped same-simple-name types from colliding (see
//! `typedef_ids_coexist`), but it did not — and must not — stop a genuine
//! disagreement: two definitions with different shapes claiming a single id.
//! That is still ambiguous, and canonicalization must still refuse it rather
//! than resolve it silently.
//!
//! An id is `CRATE@MAJOR::module::path::Name`, and the only segment an author
//! chooses is the last, so the only way to reach this from source is to point
//! two declarations in one module at one name. `Decoy` does that with
//! `name = "Colliding"`, giving it the id of the real `Colliding` beside it
//! with a different shape.
//!
//! Both defs must also survive shaking for the conflict to be reachable:
//! `Colliding` because a retained method takes it, `Decoy` because `export`
//! makes it a root. gnaw's `fixtures-negative:typedef-conflict` task asserts
//! the build fails and names the id.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

use thruput_sdk::*;

/// Referenced by the retained method below, so its def is reachable.
#[contract_type]
pub struct Colliding {
    pub limit: u32,
}

/// Claims `Colliding`'s id — same crate, same module, same final segment —
/// with a different shape. `export` makes it a spec root so it is retained
/// rather than shaken.
#[contract_type(export, name = "Colliding")]
pub struct Decoy {
    pub flag: u64,
}

pub struct CollisionContract;

#[contract]
impl CollisionContract {
    pub fn configure(cfg: Colliding) -> u32 {
        cfg.limit
    }
}
