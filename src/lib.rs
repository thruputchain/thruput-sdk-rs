#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README-lib.md")]
// #![doc(html_logo_url = "../../../etc/groundhog.png")]

#[cfg(feature = "alloc")]
extern crate alloc;

mod rt;

#[cfg(all(feature = "alloc", target_family = "wasm"))]
pub mod allocator;

#[cfg(all(not(feature = "std"), not(target_family = "wasm")))]
compile_error!("building with nostd (or just the alloc feature) is not supported outside of wasm");

#[doc(inline)]
pub use ::macros;

// Base
pub mod syscall;
pub mod types;
#[doc(inline)]
pub use syscall::bindings as sys;
pub mod crypto;
pub mod deploy;
#[cfg(feature = "alloc")]
pub mod hash;
pub mod invoke;
pub mod log;
pub mod serialize {
    #[cfg(feature = "alloc")]
    pub use ::bcs::to_bytes;
    pub use ::bcs::{from_bytes, from_bytes_seed};
    pub use ::serde::{de::DeserializeOwned, Deserialize, Serialize};
}
pub mod contract_result;
pub mod contract_test;

// Lower-level
pub mod bytestring;
pub mod delete;
pub mod nonnegative_int;
pub mod retrocausal_queue;

// #[cfg(feature = "alloc")]
pub mod result;

#[cfg(feature = "alloc")]
pub mod internal;

// Storage layer
#[cfg(feature = "alloc")]
pub mod storage;

#[cfg(feature = "mock")]
pub use ::groundhog_xdr::ffi_enums::*;

#[cfg(feature = "mock")]
pub use ::native_exec::{
    mock_contract::MockContract,
    test_environment::TestEnvironment,
};

// Available from any prelude
mod prelude_base {
    #[doc(inline)]
    pub use super::types::*;

    #[doc(inline)]
    pub use super::syscall::SysErr;

    #[doc(inline)]
    pub use super::sys::SyscallErrorCode;

    pub use thiserror::Error;

    #[doc(inline)]
    pub use super::log::*;

    #[doc(inline)]
    pub use super::invoke::*;

    #[cfg(feature = "alloc")]
    #[doc(inline)]
    pub use super::hash::*;

    #[doc(inline)]
    pub use super::serialize::{self, Deserialize, Serialize};

    #[doc(inline)]
    pub use ::xdrs::{ReadXDR, WriteXDR};

    #[doc(inline)]
    pub use super::macros::contract;

    #[doc(inline)]
    pub use super::contract_result::*;
}

/// Lower-level contract facilities
pub mod prelude {
    #[doc(inline)]
    pub use super::prelude_base::*;

    #[doc(inline)]
    pub use super::deploy::*;

    #[doc(inline)]
    pub use super::nonnegative_int::*;

    #[doc(inline)]
    pub use super::bytestring::*;

    #[doc(inline)]
    pub use super::retrocausal_queue::*;
}

/// Storage-layer facilities
#[cfg(feature = "alloc")]
pub mod storage_prelude {
    #[doc(inline)]
    pub use super::prelude_base::*;

    #[doc(inline)]
    pub use super::storage::*;

    #[doc(inline)]
    pub use super::macros::Storage;
}
