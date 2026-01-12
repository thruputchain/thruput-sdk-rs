#![feature(maybe_uninit_array_assume_init)]
#![feature(maybe_uninit_slice)]
#![feature(core_io_borrowed_buf)]
#![cfg_attr(
    not(any(doc, target_family = "wasm")),
    feature(local_key_cell_update)
)]
// #![doc(html_logo_url = "groundhog.png")]
#![doc = include_str!("../doc/lib.md")]
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

pub mod internal; // Public because used in contract macros

// Syscall wrappers
pub mod auth_claim;
pub mod block;
pub mod bytestring;
pub mod contract_op;
pub mod crypto;
pub mod delete;
pub mod deploy;
pub mod event;
pub mod hash;
pub mod hasher;
pub mod hex;
pub mod invoke;
pub mod invoke_config;
pub mod invoke_error;
pub mod log;
pub mod non_negative_int;
pub mod precompiles;
pub mod retrocausal_queue;
pub mod rt;
pub mod serialize;
pub mod stats;
pub mod types;

pub mod contract_result;
pub mod sysresult;

mod cell;
mod local;

// Direct exports

pub use auth_claim::*;
pub use bytestring::*;
pub use cell::*;
pub use contract_op::*;
pub use delete::*;
pub use deploy::*;
pub use event::*;
pub use hash::*;
pub use hasher::*;
pub use hex::*;
pub use invoke::*;
pub use log::*;
pub use non_negative_int::*;
pub use retrocausal_queue::*;
pub use rt::*;
pub use serialize::*;
pub use types::*;

pub use abort::{AbortPayload, abort, default_abort_handler, with_debug_context};
pub use abort::with_handler as with_abort_handler;

#[doc = include_str!("../doc/macro-contract.md")]
pub use macros::contract;
// These must be enumerated to avoid stomping on the above docstrings
pub use macros::{hash_to_array, sha256_static, static_key};

#[cfg(feature = "alloc")]
pub use alloc::{format, string::*, vec, vec::Vec};

#[cfg(feature = "spec")]
#[rustfmt::skip]
pub mod spec;

// "Storage" module

#[cfg(feature = "storage")]
pub mod storage;

#[cfg(feature = "storage")]
pub use macros::Storage;

#[cfg(feature = "storage")]
#[doc = include_str!("../doc/macro-storage.md")]
pub use macros::storage;

// Test environment

#[cfg(feature = "mock")]
pub mod syscall_bindings;

#[cfg(feature = "mock")]
// This is used only by contract_test
pub mod contract_test_handle;

#[cfg(feature = "mock")]
pub mod contract_test;

#[cfg(feature = "mock")]
pub use native_exec::{
    mock_contract::MockContract,
    test_environment::{TestEnvironment, TestEnvironmentHandle},
};

#[cfg(feature = "std")]
pub use groundhog_xdr::{exec_result::ExecResult, ffi_enums::*};
