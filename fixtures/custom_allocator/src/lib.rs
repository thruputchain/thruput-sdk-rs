//! A contract built with `thruput-sdk/alloc-api` that supplies its own
//! `#[global_allocator]` in its rv32 bin wrapper (src/main.rs).
//!
//! The methods allocate in patterns that exhaust the runtime's default bump
//! allocator, which never reuses memory, but fit in the heap when the most
//! recent allocation can be freed or grown in place. The rv32 tests size them
//! from [`CustomAllocator::heap_bytes`].

#![cfg_attr(not(feature = "dev"), no_std)]

extern crate alloc;

thruput_sdk::declare_mock_contract!();

mod implementation;

pub use implementation::*;
