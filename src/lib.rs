#![doc = include_str!("../README-lib.md")]
// #![doc(html_logo_url = "groundhog.png")]

#![cfg_attr(not(feature = "std"), no_std)]

#![cfg_attr(doc, deny(warnings))]

#![deny(warnings)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod rt;

#[cfg(any(doc, all(feature = "alloc", target_family = "wasm")))]
pub mod allocator;

#[cfg(all(not(feature = "std"), not(target_family = "wasm")))]
compile_error!("building with nostd (or just the alloc feature) is \
                not supported outside of wasm");

// Syscall utilities
#[cfg(feature = "mock")]
pub mod syscall_bindings;
pub mod sysresult;

// Syscall wrappers
pub mod contract_op;
pub mod bytestring;
pub mod delete;
pub mod nonnegative_int;
pub mod retrocausal_queue;
pub mod stats;

pub mod types;
pub mod crypto;
pub mod deploy;
pub mod hash;
pub mod handle;
pub mod invoke;
pub mod invoke_config;
pub mod invoke_error;
pub mod log;
pub mod precompiles;
pub mod serialize;

pub mod contract_result;
pub mod contract_test;

#[cfg(any(doc, feature = "storage"))]
pub mod storage;

// Direct exports

//pub use syscall::*;
//pub use syscall_bindings as sys;
pub use groundhog_xdr::syscalls as sys;
pub use groundhog_xdr::syscalls::SysErr;
pub use groundhog_xdr::syscall_error::SyscallErrorCode;
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
pub use xdrs::{ReadXDR, WriteXDR};

pub use macros::contract;
pub use macros::static_key;

#[cfg(feature = "alloc")]
pub use alloc::{vec, vec::Vec, format};

#[cfg(feature = "alloc")]
pub use alloc::string::*;

#[cfg(feature = "spec")]
pub mod spec;

#[cfg(feature = "mock")]
pub use groundhog_xdr::ffi_enums::*;

#[cfg(feature = "mock")]
pub use native_exec::{
    mock_contract::MockContract,
    test_environment::TestEnvironment,
};

#[cfg(feature = "storage")]
pub use macros::Storage;
#[cfg(feature = "storage")]
pub use macros::storage;
