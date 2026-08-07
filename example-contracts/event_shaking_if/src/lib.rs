//! Interface definitions for the cross-crate event-shaking example.
//!
//! Defines two traits: `Adopted` (emits `AdoptedEvent`) and `Orphan` (emits
//! `OrphanEvent`).  The impl crate (`event_shaking_subset`) implements only
//! `Adopted`, so spec shaking must drop `OrphanEvent` and its named-type
//! dependency `OrphanPayload` from the final canonicalized spec.
//!
//! Both events carry a `#[contract_type]` payload so the fixture asserts
//! typedef *retention* (`AdoptedPayload` present) as well as typedef *shaking*
//! (`OrphanPayload` absent).

#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

/// A named payload type carried by `AdoptedEvent`.  Because `AdoptedEvent`
/// is retained (the `Adopted` trait IS implemented), this typedef must
/// survive spec shaking — exercising positive typedef retention.
#[contract_type]
pub struct AdoptedPayload {
    pub value: u32,
}

/// A named payload type used only by `OrphanEvent`.  Because `OrphanEvent`
/// is shaken (the `Orphan` trait is never implemented), this typedef must
/// also disappear from the final spec — exercising reachability-based
/// type-def shaking.
#[contract_type]
pub struct OrphanPayload {
    pub code: u32,
}

/// Emitted by the `Adopted` interface; must survive spec shaking.
/// Carries `AdoptedPayload` to assert positive typedef retention.
#[derive(Event)]
pub struct AdoptedEvent {
    pub payload: AdoptedPayload,
}

/// Emitted only by the `Orphan` interface; must be shaken from the spec of
/// any contract that does not implement `Orphan`.
#[derive(Event)]
pub struct OrphanEvent {
    pub payload: OrphanPayload,
}

/// The trait that `event_shaking_subset` implements.  Its `AdoptedEvent`
/// must appear in the final spec.
#[contract(emits(AdoptedEvent))]
pub trait Adopted {
    fn adopted_ping() -> u32;
}

/// The trait that `event_shaking_subset` does NOT implement.  Its
/// `OrphanEvent` (and the `OrphanPayload` typedef) must be absent from the
/// final spec.
#[contract(emits(OrphanEvent))]
pub trait Orphan {
    fn orphan_ping() -> u32;
}
