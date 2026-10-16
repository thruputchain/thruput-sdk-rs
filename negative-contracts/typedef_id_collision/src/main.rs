//! RV32 binary wrapper for the same-id collision negative fixture. The ELF
//! links, but the spec canonicalization the `groundhog` CLI runs during
//! lowering must FAIL with a typedef conflict.

#![no_std]
#![no_main]

extern crate typedef_id_collision;
