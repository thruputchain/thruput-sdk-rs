//! RV32 spec-carrier binary for the ERC20 standard interface.
//!
//! This is a degenerate, zero-entrypoint "contract": a trait registers no
//! dispatch methods, so the only thing this artifact carries is the ERC20
//! interface's `groundhogspecrawv0` fragments. It exists so the interface can
//! be built into an rv32 ELF and have its spec extracted with
//! `groundhog-spec-tool spec --elf`.
#![no_std]
#![no_main]

// Link the interface library so its macro-emitted `#[used]` spec section is
// retained in the final ELF for extraction. `_start`, panic handler,
// allocator, and linker script come transitively via sdk → groundhog-rv32-runtime
// (sdk force-links it with `extern crate groundhog_rv32_runtime as _`).
extern crate groundhog_erc20_if;
