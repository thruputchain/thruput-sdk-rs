//! RV32 binary wrapper for the interface-declarer collision negative fixture.
//! The ELF links, but the spec canonicalization the `thruput` CLI runs
//! during lowering must FAIL: two declarers anchor one interface key.

#![no_std]
#![no_main]

extern crate iface_declarer_collision;
