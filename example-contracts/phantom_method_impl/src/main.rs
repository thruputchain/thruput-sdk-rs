//! RV32 binary wrapper for the `phantom_method_impl` contract.
//!
//! `thruput build` derives `--bin phantom_method_impl` from the package
//! name, so this target is what produces the deployable ELF.
//!
//! The `extern crate` line force-links this package's own rlib, so the
//! entrypoint records land in the binary. The runtime (`_start`,
//! `#[panic_handler]`, `#[global_allocator]`, linker script) arrives
//! transitively: `sdk` force-links `groundhog-rv32-runtime` on rv32 targets.

#![no_std]
#![no_main]

extern crate phantom_method_impl;
