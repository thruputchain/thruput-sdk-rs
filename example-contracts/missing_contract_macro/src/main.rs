//! RV32 binary wrapper for the missing-`#[contract]` negative fixture.
//!
//! It links, but registers NO entrypoint records, so the artifact cannot be
//! invoked. `check_rv32_missing_entrypoints_detected` in scripts/dev.sh
//! asserts the empty-dispatch-table guard fires on exactly this shape.

#![no_std]
#![no_main]

extern crate missing_contract_macro;
