//! RV32 binary wrapper for the `custom_allocator` contract.
//!
//! `thruput build` derives `--bin custom_allocator` from the package name, so
//! this target produces the ELF intermediate.
//!
//! The `extern crate` line force-links this package's own rlib, so the
//! entrypoint records land in the binary. `_start`, the panic handler and the
//! linker script arrive through `thruput-sdk`. The SDK is built with
//! `alloc-api`, so the runtime installs no allocator, and this file declares
//! one. A bin wrapper is the right place: it builds only for the rv32
//! artifact, so native tests keep the host allocator and contracts that
//! depend on this one do not inherit it.

#![no_std]
#![no_main]

extern crate custom_allocator;

mod allocator;

#[global_allocator]
static ALLOCATOR: allocator::ReclaimingBump = allocator::ReclaimingBump::new();
