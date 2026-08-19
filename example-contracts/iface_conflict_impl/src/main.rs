//! RV32 binary wrapper for the anchored interface-metadata-conflict negative
//! fixture. The ELF links, but the spec canonicalization the `groundhog` CLI
//! runs during lowering must FAIL with a `Token` emits-metadata conflict.

#![no_std]
#![no_main]

extern crate iface_conflict_impl;
