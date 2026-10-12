//! RV32 binary wrapper for the `test_bytestring` contract.
//!
//! `thruput build` derives `--bin test_bytestring` from the package name, so
//! this target is what produces the deployable ELF.
//!
//! This contract registers its entrypoints by hand rather than through the
//! `#[contract]` macro; see lib.rs.
//!
//! The `extern crate` line force-links this package's own rlib, so the
//! entrypoint records land in the binary. The runtime (`_start`,
//! `#[panic_handler]`, `#[global_allocator]`, linker script) arrives
//! transitively: `sdk` force-links `thruput-rv32-runtime` on rv32 targets.

#![no_std]
#![no_main]

extern crate test_bytestring;
