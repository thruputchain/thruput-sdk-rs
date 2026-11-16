//! RV32 binary wrapper for the `mem_intrinsics` contract.
//!
//! `thruput build` derives `--bin mem_intrinsics` from the package name, so
//! this target is what produces the deployable ELF.
//!
//! The `extern crate` line force-links this package's own rlib, so the
//! entrypoint records land in the binary. The runtime, with the `memcpy`,
//! `memmove` and `memset` this contract measures, arrives transitively:
//! `thruput-sdk` force-links `thruput-rv32-runtime` on rv32 targets.

#![no_std]
#![no_main]

extern crate mem_intrinsics;
