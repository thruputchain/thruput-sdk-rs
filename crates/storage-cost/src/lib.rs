//! Storage prices for Thruput contracts: [`StorageCostModel`] wraps a
//! network's cost model, which prices data items, bytestring bytes,
//! deployments, auth keys and queue entries. The native test environment
//! configures one.

#![no_std]

mod model;
pub use model::*;
