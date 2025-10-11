#![feature(maybe_uninit_array_assume_init)]
#![feature(maybe_uninit_slice)]
#![feature(core_io_borrowed_buf)]
// #![doc(html_logo_url = "groundhog.png")]
#![doc = include_str!("../README-lib.md")]
#![cfg_attr(doc, deny(warnings))]
#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(all(not(feature = "std"), not(target_family = "wasm")))]
compile_error!("Except when building a wasm, sdk feature \"std\" is required");

#[cfg(any(doc, all(target_family = "wasm", feature = "alloc")))]
pub mod allocator;

//// Install an allocator that just panics
// #[cfg(all(target_family = "wasm", not(feature = "alloc")))]
// pub mod allocator_panic;

#[cfg(all(feature = "dynamic_bufs", not(feature = "alloc")))]
compile_error!(
    "SDK feature \"dynamic_bufs\" requires liballoc, provided by
    SDK feature \"alloc\"."
);
// We avoid silently turning on "alloc", which may not be desired

pub mod internal;
// (Public because used in contract macros)

// Syscall wrappers
pub mod bytestring;
pub mod contract_op;
pub mod delete;
pub mod nonnegative_int;
pub mod retrocausal_queue;
pub mod stats;

pub mod crypto;
pub mod deploy;
pub mod hash;
pub mod invoke;
pub mod invoke_config;
mod invoke_encode; // TODO: make this a sub-module of invoke
pub mod invoke_error;
pub mod log;
pub mod precompiles;
pub mod rt;
pub mod serialize;
pub mod types;

pub mod contract_result;
pub mod sysresult;

#[cfg(feature = "storage")]
pub mod storage;

// Direct exports

pub use bytestring::*;
pub use contract_op::*;
pub use deploy::*;
pub use invoke::*;
pub use log::*;
pub use nonnegative_int::*;
pub use retrocausal_queue::*;

pub use hash::*;
pub use rt::*;
pub use types::*;
pub use xdrs::{ReadXDR, ReadXDROwned, WriteXDR};

pub use macros::contract;
pub use macros::static_key;
#[cfg(feature = "storage")]
pub use macros::{storage, Storage};

#[cfg(feature = "alloc")]
pub use alloc::{format, string::*, vec, vec::Vec};

#[cfg(feature = "spec")]
#[rustfmt::skip]
pub mod spec;

// Test environment

#[cfg(feature = "mock")]
pub mod syscall_bindings;

#[cfg(feature = "mock")]
// This is used only by contract_test
pub mod handle;

#[cfg(feature = "mock")]
pub mod contract_test;

#[cfg(feature = "mock")]
pub use {
    groundhog_xdr::ffi_enums::*,
    native_exec::{
        mock_contract::MockContract, test_environment::TestEnvironment,
    },
};
