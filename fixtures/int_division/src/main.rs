//! RV32 binary wrapper for the `int_division` contract.
//!
//! `thruput build` derives `--bin int_division` from the package name, so
//! this target is what produces the deployable ELF.
//!
//! The `extern crate` line force-links this package's own rlib, so the
//! entrypoint records land in the binary. The runtime, with the division
//! routines this contract measures, arrives transitively: `thruput-sdk`
//! force-links `thruput-rv32-runtime` on rv32 targets.

#![no_std]
#![no_main]

extern crate int_division;
