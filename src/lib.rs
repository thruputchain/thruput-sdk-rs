#![no_std]
#![doc = include_str!("../README-lib.md")]
// #![doc(html_logo_url = "../../../etc/groundhog.png")]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

mod rt;

#[cfg(all(feature = "alloc", target_family = "wasm"))]
pub mod allocator;

#[doc(inline)]
pub use ::macros;

// Base
pub mod syscall;
pub mod types;
#[doc(inline)]
pub use syscall::bindings as sys;
pub mod crypto;
pub mod deploy;
pub mod hash;
pub mod invoke;
pub mod log;
pub mod serialize {
    pub use ::bcs::{from_bytes, from_bytes_seed, to_bytes};
    pub use ::serde::{de::DeserializeOwned, Deserialize, Serialize};
}
pub mod contract_result;
pub mod contract_test;

// Lower-level
pub mod bytestring;
pub mod delete;
pub mod nonnegative_int;
// pub mod retrocausal_queue;

// #[cfg(feature = "alloc")]
pub mod result;

#[cfg(feature = "alloc")]
pub mod internal;

// Storage layer
#[cfg(feature = "alloc")]
pub mod storage;

#[cfg(feature = "mock")]
pub use ::groundhog_xdr::ffi_enums::*;

// #[cfg(feature = "mock")]
// pub use ::native_exec::*;

// Available from any prelude
mod prelude_base {
    #[doc(inline)]
    pub use super::types::{
        ContractAddress, Ed25519PublicKey, Ed25519Signature, PublicKey,
        SHA256Hash, Signature, StorageKey,
    };

    #[doc(inline)]
    pub use super::syscall::SysErr;

    #[doc(inline)]
    pub use super::sys::SyscallErrorCode;

    pub use thiserror::Error;

    #[doc(inline)]
    pub use super::log::*;

    #[doc(inline)]
    pub use super::invoke::*;

    #[doc(inline)]
    pub use super::hash::*;

    #[doc(inline)]
    pub use super::serialize::{self, Deserialize, Serialize};

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

    // #[doc(inline)]
    // pub use super::retrocausal_queue::*;
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
