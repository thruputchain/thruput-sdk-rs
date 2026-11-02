//! RV32 binary wrapper for the event-signature collision negative fixture.
//! The ELF links, but the spec canonicalization the `thruput` CLI runs
//! during lowering must FAIL: the two events cannot be told apart on the wire.

#![no_std]
#![no_main]

extern crate event_signature_collision;
