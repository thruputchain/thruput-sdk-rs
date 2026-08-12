// #![doc(html_logo_url = "groundhog.png")]
#![doc = include_str!("../doc/lib.md")]
#![cfg_attr(doc, deny(warnings))]
#![no_std]
#![allow(clippy::result_large_err)]
#![allow(clippy::large_enum_variant)]

#[cfg(all(
    feature = "std",
    any(target_family = "wasm", target_arch = "riscv32"),
))]
compile_error!(
    "Feature 'std' cannot be enabled when targeting guest contracts"
);

#[cfg(all(
    not(feature = "std"),
    not(target_family = "wasm"),
    not(target_arch = "riscv32"),
))]
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

/// 256-bit unsigned integer arithmetic.
///
/// [`U256`] behaves like `u128`, including on overflow: the operators wrap
/// when `overflow-checks` is off (as in every contract profile) and panic when
/// it is on. See [`u256::mul_div_u128`] for the exact `a * b / d` that motivates
/// most reaches for a 256-bit type.
pub use groundhog_u256 as u256;
pub use groundhog_u256::{U256, checked_mul_div_u128, mul_div_u128};

#[cfg(feature = "alloc")]
extern crate alloc;

// Guest targets get their runtime — `#[global_allocator]`, `#[panic_handler]`,
// and (on rv32) the `_start` entry — from a dedicated runtime crate, the same
// way a hosted target gets these from std + the system. Force-link it so
// contracts (and the rv32 bin wrapper) don't have to name it. The
// `extern crate ... as _` is load-bearing: nothing references these items by
// name, so without it the runtime would be pruned as an unused dependency,
// taking the lang items with it.
#[cfg(target_arch = "wasm32")]
extern crate groundhog_wasm_runtime as _;

#[cfg(target_arch = "riscv32")]
extern crate groundhog_rv32_runtime as _;

// Guest-dispatch plumbing for the `#[contract]` macro on rv32. The macro
// emits `#[link_section = "groundhog_rv32_entrypoints"]` Entrypoint statics;
// this re-export gives that generated code a stable path.
#[cfg(target_arch = "riscv32")]
#[doc(hidden)]
pub mod __rv32 {
    pub use groundhog_rv32_runtime::Entrypoint;
}

#[cfg(not(any(target_family = "wasm", target_arch = "riscv32")))]
#[doc(hidden)]
pub extern crate linkme as __linkme;

#[doc(hidden)]
pub mod internal;

mod cell;
mod diagnostics;
mod local;
mod panic;
mod re_throw;
mod throw;
mod util;
pub mod xdr;

// Primitive storage types: interfaces returning Result
pub mod sys_result;

// Primitive storage types: interfaces throwing exceptions
#[doc(hidden)]
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
pub mod stats;
pub mod types;

#[doc(hidden)]
pub mod mintable;

pub use auth_claim::*;
pub use block::*;
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
pub use mintable::*;
pub use panic::*;
pub use re_throw::*;
pub use receive_auth_handles::*;
pub use rt::*;
pub use serialize::*;
pub use signer::*;
pub use throw::*;
pub use types::*;
pub use util::*;
// XDR serialization vocabulary that is part of the SDK's public API (macro
// expansions reference these as `::sdk::…`). The rest of the xdrs surface
// stays namespaced under `sdk::xdr`.
pub use xdr::{
    ReadError, ReadResult, ReadXDR, Reader, WriteError, WriteResult, WriteXDR,
    Writer,
};

#[allow(unused_imports)]
pub use diagnostics::*;

#[doc = include_str!("../doc/macro-contract.md")]
pub use groundhog_macros::contract;
// These must be enumerated to avoid stomping on the above docstring
pub use groundhog_macros::ContractTypeSpec;
pub use groundhog_macros::Event;
pub use groundhog_macros::contract_exception;
pub use groundhog_macros::contract_type;
pub use groundhog_macros::exn;
pub use groundhog_macros::export_interface_spec;
pub use groundhog_macros::{
    hash_to_array, sha256_array_ident, sha256_ident, sha256_static, static_key,
};

pub use groundhog_contract_spec::ContractTypeName;

// "Storage" facility

pub mod storage;

/// Declare a native test asserting that no two contract state roots
/// **linked into this binary** share a storage domain.
///
/// The per-crate test generated by `#[contract]` only sees its own crate's
/// roots. A guest binary that links several contract crates (the rv32
/// bin-wrapper pattern) aggregates all of them — put this in the wrapper
/// crate's `tests/` alongside a `use <contract_crate> as _;` for each
/// linked contract crate so their registrations are linked in:
///
/// ```ignore
/// use counter as _;
/// sdk::storage_domain_uniqueness_test!();
/// ```
#[macro_export]
macro_rules! storage_domain_uniqueness_test {
    () => {
        #[cfg(all(
            test,
            not(any(target_family = "wasm", target_arch = "riscv32"))
        ))]
        #[test]
        fn storage_domains_unique_across_linked_crates() {
            $crate::storage::assert_unique_storage_domains();
        }
    };
}

pub use groundhog_macros::Deletable;

pub use groundhog_macros::Storage;

#[doc = include_str!("../doc/macro-storage.md")]
pub use groundhog_macros::storage;

// Mock contract execution environment

mod mock;

#[cfg(feature = "mock")]
pub use mock::*;
