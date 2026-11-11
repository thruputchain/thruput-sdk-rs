//! RV32 binary wrapper for the `custom_allocator` contract.
//!
//! `thruput build` derives `--bin custom_allocator` from the package name, so
//! this target produces the ELF intermediate.
//!
//! The `extern crate` line force-links this package's own rlib. Under
//! `alloc-api` the runtime installs no allocator, so this file declares one;
//! see "Custom allocator" in the SDK guide (`crates/sdk/doc/lib.md`).

#![no_std]
#![no_main]

extern crate custom_allocator;

mod allocator;

#[global_allocator]
static ALLOCATOR: allocator::ReclaimingBump = allocator::ReclaimingBump::new();
