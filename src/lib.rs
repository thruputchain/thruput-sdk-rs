// #![doc(html_logo_url = "groundhog.png")]
#![doc = include_str!("../doc/lib.md")]
#![cfg_attr(doc, deny(warnings))]
#![no_std]
#![allow(clippy::result_large_err)]
#![allow(clippy::large_enum_variant)]

#[cfg(all(feature = "std", target_family = "wasm"))]
compile_error!("Feature 'std' cannot be enabled when targetting wasm");

#[cfg(all(not(feature = "std"), not(target_family = "wasm")))]
compile_error!("Feature 'std' is required when compiling natively");

// Allow `#[exn]` to emit `::sdk::exn::Exn` and have it resolve
// inside this crate itself. (Same pattern as serde.)
extern crate self as sdk;

// Re-export the xdrs crate so that derive macros (ReadXDR, WriteXDR) can
// reference internal xdrs module paths via `::sdk::__xdrs::…` — this lets
// contracts depend only on `sdk`, not on `xdrs` directly.
#[doc(hidden)]
pub extern crate xdrs as __xdrs;

#[cfg(feature = "std")]
extern crate std;

pub use groundhog_contract_spec as contract_spec;
#[doc(hidden)]
pub use groundhog_contract_spec as __contract_spec;

#[cfg(feature = "mock")]
#[doc(hidden)]
pub extern crate linkme as __linkme;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(any(doc, all(target_family = "wasm", feature = "alloc")))]
pub mod allocator;

#[doc(hidden)]
pub mod internal;

mod cell;
mod debug;
#[cfg(feature = "debugging")]
mod debug_context;
mod local;
mod panic;
mod re_throw;
mod throw;
mod util;
mod xdr;

// Primitive storage types: interfaces returning Result
#[doc(hidden)]
pub mod sys_result;

// Primitive storage types: interfaces throwing exceptions
pub mod sys_exn;

// Currently-blessed primitive interfaces:
pub use sys_exn::bytestring::*;
pub use sys_exn::delete::*;
pub use sys_exn::nnint::*;
pub use sys_exn::rcq::*;

#[doc(hidden)]
pub mod call;
pub use call::ContractClient;

// TODO: Make these modules pub(crate) at most, in order to allow
// reorganization without breaking changes.
pub mod auth_claim;
pub mod block;
pub mod collect_auth_handles;
pub mod contract_op;
pub mod crypto;
pub mod deploy;
pub mod event;
pub mod exn;
pub mod hash;
pub mod hasher;
pub mod hex;
pub mod invoke_config;
pub mod log;
pub mod precompiles;
pub mod receive_auth_handles;
pub mod rt;
pub mod serialize;
pub mod signer;
pub mod signer_mapper;
pub mod stats;
pub mod types;

pub use auth_claim::*;
pub use cell::*;
pub use collect_auth_handles::*;
pub use contract_op::*;
pub use deploy::*;
pub use event::*;
pub use exn::*;
pub use hash::*;
pub use hasher::*;
pub use hex::*;
pub use invoke_config::*;
pub use log::*;
pub use panic::*;
pub use re_throw::*;
pub use receive_auth_handles::*;
pub use rt::*;
pub use serialize::*;
pub use signer::*;
pub use throw::*;
pub use types::*;
pub use util::*;
pub use xdr::*;

#[cfg(feature = "debugging")]
pub use debug_context::with_debug_context;

#[cfg(feature = "debugging")]
pub use ::thiserror::Error;

#[doc = include_str!("../doc/macro-contract.md")]
pub use groundhog_macros::contract;
// These must be enumerated to avoid stomping on the above docstrings
pub use groundhog_contract_spec::ContractTypeName;
pub use groundhog_macros::ContractTypeSpec;
pub use groundhog_macros::Event;
pub use groundhog_macros::contract_exception;
pub use groundhog_macros::contracttype;
pub use groundhog_macros::exn;
pub use groundhog_macros::{hash_to_array, sha256_static, static_key};

#[cfg(feature = "alloc")]
pub use alloc::{format, string::*, vec, vec::Vec};

pub mod mintable;

pub use mintable::*;

#[cfg(feature = "spec")]
#[rustfmt::skip]
pub mod spec;

// "Storage" facility

#[cfg(feature = "storage")]
pub mod storage;

#[cfg(feature = "storage")]
pub use groundhog_macros::Deletable;

#[cfg(feature = "storage")]
pub use groundhog_macros::Storage;

#[cfg(feature = "storage")]
#[doc = include_str!("../doc/macro-storage.md")]
pub use groundhog_macros::storage;

// Development environment

#[cfg(feature = "mock")]
#[doc(hidden)]
#[path = "dev/mock_registry.rs"]
pub mod mock_registry;

#[cfg(feature = "dev")]
mod dev;

#[cfg(feature = "dev")]
pub use dev::*;

#[macro_export]
macro_rules! declare_mock_contract {
    () => {
        #[cfg(feature = "dev")]
        #[doc(hidden)]
        pub fn register_mock_contract(
            env: &::sdk::TestEnvironmentHandle,
        ) -> ::sdk::MockContract {
            ::sdk::register_mock_package(env, env!("CARGO_PKG_NAME"))
        }
    };
}
