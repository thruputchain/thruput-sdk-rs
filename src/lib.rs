#![cfg_attr(
    not(any(doc, target_family = "wasm")),
    feature(local_key_cell_update)
)]
// #![doc(html_logo_url = "groundhog.png")]
#![doc = include_str!("../doc/lib.md")]
#![cfg_attr(doc, deny(warnings))]
#![no_std]
#![allow(deprecated)] // XXX

#[cfg(all(feature = "std", target_family = "wasm"))]
compile_error!("Feature 'std' cannot be enabled when targetting wasm");

#[cfg(all(not(feature = "std"), not(target_family = "wasm")))]
compile_error!("Feature 'std' is required when compiling natively");

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(any(doc, all(target_family = "wasm", feature = "alloc")))]
pub mod allocator;

#[doc(hidden)]
pub mod internal;

mod cell;
mod debug;
mod local;
mod panic;
mod xdr;

pub mod call;

// TODO: Make these modules pub(crate) in order to allow
// reorganization without breaking changes.

pub mod auth_claim;
pub mod block;
pub mod bytestring;
pub mod contract_op;
pub mod contract_result;
pub mod crypto;
pub mod delete;
pub mod deploy;
pub mod event;
pub mod exn;
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
pub mod sysresult;
pub mod types;

pub use auth_claim::*;
pub use bytestring::*;
pub use cell::*;
pub use contract_op::*;
pub use delete::*;
pub use deploy::*;
pub use event::*;
pub use exn::*;
pub use hash::*;
pub use hasher::*;
pub use hex::*;
pub use invoke::*;
pub use log::*;
pub use non_negative_int::*;
pub use panic::*;
pub use retrocausal_queue::*;
pub use rt::*;
pub use serialize::*;
pub use types::*;

pub use xdr::*;

pub use abort::{
    AbortInfo, AbortPayload, Never, abort, abort_with_info, with_abort_handler,
};

#[cfg(feature = "call_tracking")]
pub use abort::with_debug_context;

#[doc = include_str!("../doc/macro-contract.md")]
pub use groundhog_macros::contract;
// These must be enumerated to avoid stomping on the above docstrings
pub use groundhog_macros::{hash_to_array, sha256_static, static_key};

#[cfg(feature = "alloc")]
pub use alloc::{format, string::*, vec, vec::Vec};

#[cfg(feature = "spec")]
#[rustfmt::skip]
pub mod spec;

// "Storage" facility

#[cfg(feature = "storage")]
pub mod storage;

#[cfg(feature = "storage")]
pub use groundhog_macros::Storage;

#[cfg(feature = "storage")]
#[doc = include_str!("../doc/macro-storage.md")]
pub use groundhog_macros::storage;

// Development environment

#[cfg(feature = "dev")]
mod dev;

#[cfg(feature = "dev")]
pub use dev::*;

#[cfg(feature = "dev")]
pub mod syscall_bindings;
