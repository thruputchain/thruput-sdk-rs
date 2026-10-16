//! rv32 spec-carrier binary for the SignedOp standard interface.
//!
//! A trait's interface spec is a *candidate*: post-processing keeps it only if
//! something anchors it. `export_interface_spec!` emits that anchor; the
//! interface/method fragments come from the linked `thruput-signed-op-if`
//! lib. Building this crate for rv32 yields the SignedOp interface's full
//! spec, which `groundhog codegen ts` turns into client bindings.
//!
//! Gated behind the `entrypoints` feature (see Cargo.toml), so a plain
//! `cargo build`/`test` skips this rv32-only, entry-point-less binary.
#![no_std]
#![no_main]

extern crate thruput_signed_op_if;

// Anchor the SignedOp interface so its candidate spec survives shaking.
thruput_sdk::export_interface_spec!(thruput_signed_op_if::SignedOp);
