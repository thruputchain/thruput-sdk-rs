//! RV32 binary wrapper for the missing-`#[contract]` negative fixture.
//!
//! It links, but registers NO entrypoint records, so the artifact cannot be
//! invoked. gnaw's `fixtures-negative:missing-entrypoints` task asserts the
//! empty-dispatch-table guard fires on exactly this shape.

#![no_std]
#![no_main]

extern crate missing_contract_macro;
