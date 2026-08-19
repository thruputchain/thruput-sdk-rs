//! RV32 binary wrapper for the reachable-typedef-conflict negative fixture.
//! The ELF links, but the spec canonicalization the `groundhog` CLI runs
//! during lowering must FAIL with a `SharedConfig` typedef conflict.

#![no_std]
#![no_main]

extern crate typedef_collision_reachable;
