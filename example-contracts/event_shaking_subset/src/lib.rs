//! Cross-crate event-shaking example contract.
//!
//! Implements only the `Adopted` trait from `event_shaking_if`, leaving
//! `Orphan` unimplemented.  The spec-tool must shake `OrphanEvent` and its
//! named-type dependency `OrphanPayload` from the final canonicalized spec.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use implementation::EventShakingSubsetContract;
