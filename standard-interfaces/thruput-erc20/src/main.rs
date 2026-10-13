//! rv32 spec-carrier binary for the ERC20 standard interface.
//!
//! A trait's interface spec is a *candidate*: post-processing keeps it only if
//! something anchors it. `export_interface_spec!` emits that anchor; the
//! interface/method fragments come from the linked `thruput-erc20-if` lib
//! (retained via `#[used]` on rv32). Building this crate for rv32 yields the
//! ERC20 interface's full spec. `_start`/panic handler/allocator come via
//! thruput-sdk → thruput-rv32-runtime.
//!
//! Gated behind the `entrypoints` feature (see Cargo.toml) — built with
//! `-F entrypoints` like every other contract's spec build — so a plain
//! `cargo build`/`test` skips this rv32-only, entry-point-less binary.
#![no_std]
#![no_main]

extern crate thruput_erc20_if;

// Anchor the ERC20 interface so its candidate spec survives shaking.
thruput_sdk::export_interface_spec!(thruput_erc20_if::ERC20);
thruput_sdk::export_interface_spec!(thruput_erc20_if::ERC20AllowanceExpiry);
