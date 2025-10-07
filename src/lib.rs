#![feature(maybe_uninit_array_assume_init)]
#![feature(maybe_uninit_slice)]
#![feature(core_io_borrowed_buf)]

#![doc = include_str!("../README-lib.md")]
// #![doc(html_logo_url = "groundhog.png")]

#![cfg_attr(not(feature = "std"), no_std)]

#![cfg_attr(doc, deny(warnings))]

#![deny(warnings)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(any(doc, all(feature = "alloc", target_family = "wasm")))]
pub mod allocator;

// XXX find out what dependency is saying `extern crate alloc`, and remove this
#[cfg(all(target_family = "wasm", not(feature = "alloc")))]
pub mod allocator_panic;

#[cfg(all(not(feature = "std"), not(target_family = "wasm")))]
compile_error!("except when building a wasm, sdk feature \"std\" is required");

/*
#[cfg(all(feature = "alloc", target_family = "wasm"))]
compile_error!("FOR TESTING wasm build: alloc must be disabled");

#[cfg(all(feature = "std", target_family = "wasm"))]
compile_error!("FOR TESTING wasm build: std must be disabled");
*/

#[cfg(not(feature = "dynamic_bufs"))]
pub(crate) mod internal;

#[cfg(feature = "mock")]
pub mod syscall_bindings;

#[cfg(feature = "mock")]
// This is used only by contract_test
pub mod handle;

// Syscall wrappers
pub mod contract_op;
pub mod bytestring;
pub mod delete;
pub mod nonnegative_int;
pub mod retrocausal_queue;
pub mod stats;

pub mod rt;
pub mod types;
pub mod crypto;
pub mod deploy;
pub mod hash;
pub mod invoke;
pub mod invoke_config;
pub mod invoke_error;
pub mod log;
pub mod precompiles;
pub mod serialize;

pub mod sysresult;
pub mod contract_result;
pub mod contract_test;

#[cfg(any(doc, feature = "storage"))]
pub mod storage;

// Direct exports

pub use contract_op::*;
pub use log::*;
pub use invoke::*;
pub use deploy::*;
pub use nonnegative_int::*;
pub use bytestring::*;
pub use retrocausal_queue::*;

pub use rt::*;
pub use types::*;
pub use hash::*;
pub use xdrs::{ReadXDR, ReadXDROwned, WriteXDR};

pub use macros::contract;
pub use macros::static_key;
#[cfg(feature = "storage")]
pub use macros::{Storage, storage};

#[cfg(feature = "alloc")]
pub use alloc::{vec, vec::Vec, string::*, format};

#[cfg(feature = "spec")]
pub mod spec;

#[cfg(feature = "mock")]
pub use groundhog_xdr::ffi_enums::*;

#[cfg(feature = "mock")]
pub use native_exec::{
    mock_contract::MockContract,
    test_environment::TestEnvironment,
};
