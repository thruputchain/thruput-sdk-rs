//! RV32 binary wrapper for the duplicate-entrypoint negative fixture.
//! The link of this binary must FAIL with a duplicate symbol.

#![no_std]
#![no_main]

extern crate dup_entrypoint_neg;
