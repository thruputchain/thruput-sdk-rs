//! RV32 binary wrapper for the `unknown_method` contract.
//!
//! `thruput build` derives `--bin unknown_method` from the package name, so
//! this target is what produces the deployable ELF.

#![no_std]
#![no_main]

extern crate unknown_method;
