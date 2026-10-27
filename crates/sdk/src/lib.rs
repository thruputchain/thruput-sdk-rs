#![doc = include_str!("../doc/lib.md")]
#![cfg_attr(doc, deny(warnings))]
#![no_std]
#![allow(clippy::result_large_err)]
#![allow(clippy::large_enum_variant)]

#[cfg(all(feature = "std", target_arch = "riscv32"))]
compile_error!(
    "Feature 'std' cannot be enabled when targeting guest contracts"
);

#[cfg(all(not(feature = "std"), not(target_arch = "riscv32")))]
compile_error!("Feature 'std' is required when compiling natively");

// Allow `#[exn]` to emit `::thruput_sdk::exn::Exn` and have it resolve
// inside this crate itself. (Same pattern as serde.)
extern crate self as thruput_sdk;

// Re-export the xdrs crate so that derive macros (ReadXDR, WriteXDR) can
// reference internal xdrs module paths via `::thruput_sdk::__xdrs::…` — this
// lets contracts depend only on `thruput_sdk`, not on `xdrs` directly.
#[doc(hidden)]
pub extern crate xdrs as __xdrs;

#[cfg(feature = "std")]
extern crate std;

pub use thruput_contract_spec as contract_spec;
#[doc(hidden)]
pub use thruput_contract_spec as __contract_spec;

/// 256-bit integer arithmetic, signed and unsigned.
///
/// [`U256`] and [`I256`] behave like `u128` and `i128`, including on overflow:
/// the operators wrap when `overflow-checks` is off (as in every contract
/// profile) and panic when it is on. The native test harness builds with
/// overflow checks *on*, so an overflow that fails a test wraps silently
/// on-chain — see the [Arithmetic](crate#arithmetic) section of the crate
/// docs, and prefer `checked_*` (or the storage-level
/// [`NNInt`](crate::storage::NNInt) / [`Balance`](crate::storage::Balance)
/// guards) for money math.
///
/// For the exact `a * b / d` that motivates most reaches for a 256-bit type,
/// see [`u256::mul_div_u128`] (`u128` in, `u128` out) or [`U256::mul_div`]
/// (256-bit throughout, on an exact 512-bit intermediate). The crate also
/// carries OpenZeppelin's `Math.sol` and `SignedMath.sol` surfaces —
/// `mul_shr`, `mul_mod`, `mod_exp`, `inv_mod`, `average`, `ceil_div`,
/// `integer_sqrt`/`log2`/`log10`/`log256` with a [`Rounding`] direction —
/// under the same names bar that one (Solidity's is `sqrt`).
///
/// `U256::from` takes a `u128` and `I256::from` an `i128`, both const fns, so
/// unsuffixed literals and const items work: `const FEE: U256 =
/// U256::from(1000)`. Narrower values go through `.into()`,
/// [`U256::from_u32`]/[`U256::from_u64`], or
/// [`I256::from_i32`]/[`I256::from_i64`].
pub use thruput_u256 as u256;
pub use thruput_u256::{
    I256, Rounding, TryFromIntError, U256, checked_mul_div_u128, mul_div_u128,
};

/// A span of time, for arithmetic on [`TimePoint`].
///
/// `core`'s own type, re-exported so a contract can reach for it without
/// naming `core` itself. `thruput-xdr` implements `+` and `-` between
/// `TimePoint` and `Duration`, so a deadline reads the way it does in
/// Solidity:
///
/// ```ignore
/// let now = thruput_sdk::block::get_block_timepoint();
/// let deadline = &now + 7 * Duration::from_hours(24);
/// if now > deadline { ... }
/// ```
///
/// Those operators panic on overflow in every build profile, unlike the
/// integer primitives: a deadline that silently wrapped into the past would
/// be a security bug. [`TimePoint::checked_add`] and `saturating_add` handle
/// the edge without panicking. [`BlockHeight`] takes the same shape against
/// a signed number of blocks, which needs no import.
pub use core::time::Duration;

#[cfg(feature = "alloc")]
extern crate alloc;

// Guest targets get their runtime — `#[global_allocator]`, `#[panic_handler]`,
// and the `_start` entry — from a dedicated runtime crate, the same way a
// hosted target gets these from std + the system. Force-link it so contracts
// (and the rv32 bin wrapper) don't have to name it. The
// `extern crate ... as _` is load-bearing: nothing references these items by
// name, so without it the runtime would be pruned as an unused dependency,
// taking the lang items with it.
#[cfg(target_arch = "riscv32")]
extern crate thruput_rv32_runtime as _;

// Guest-dispatch plumbing for the `#[contract]` macro on rv32. The macro
// emits `#[link_section = "thruput_rv32_entrypoints"]` Entrypoint statics;
// this re-export gives that generated code a stable path.
#[cfg(target_arch = "riscv32")]
#[doc(hidden)]
pub mod __rv32 {
    pub use thruput_rv32_runtime::Entrypoint;

    /// The runtime dispatcher's undefined-method hook: the one definition of
    /// the symbol `thruput-rv32-runtime` tail-calls when no entrypoint
    /// matches the invoked method id.
    ///
    /// The runtime cannot build a `LibraryExn` itself — it is dependency-free
    /// by design, and `thruput_sdk` depends on *it*, not the other way round —
    /// so the two meet at this C symbol.
    ///
    /// Takes no `method_id`: the caller chose the id it invoked, so echoing
    /// it back buys nothing, and the unit variant keeps both the encoded
    /// payload and the emitted code minimal.
    #[unsafe(no_mangle)]
    extern "C" fn __thruput_unknown_method() -> ! {
        crate::exn::library_throw(crate::exn::LibraryExn::UnknownMethod)
    }
}

#[cfg(not(target_arch = "riscv32"))]
#[doc(hidden)]
pub extern crate linkme as __linkme;

#[doc(hidden)]
pub mod internal;

mod cell;
mod diagnostics;
mod local;
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
pub use sys_exn::flags::*;
pub use sys_exn::nnint::*;
pub use sys_exn::rcq::*;

#[doc(hidden)]
pub mod call;
pub use call::{ContractClient, MapExceptions};
// Error types returned by ordinary and exception-mapped `try_*` calls.
pub use call::{CallError, RawCallError};

/// Off-chain invocation through generated `<Iface>Remote` proxies.
#[cfg(feature = "remote")]
pub use thruput_contract_util::remote;

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
pub mod interface_table;
pub mod invoke_config;
pub mod log;
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
// Explicit: a glob would republish #[doc(hidden)] mintable internals.
pub use mintable::{AuthTokenPreimage, Mintable, MintableType};
pub use re_throw::*;
pub use receive_auth_handles::*;
pub use rt::*;
pub use serialize::*;
pub use signer::*;
pub use throw::*;
/// Allocation-free base58 conversion for public identifiers.
pub use thruput_xdr::base58;
pub use thruput_xdr::base58::{Base58, base58_encode};
pub use types::*;
pub use util::*;
// XDR serialization vocabulary that is part of the SDK's public API (macro
// expansions reference these as `::thruput_sdk::…`). The rest of the xdrs
// surface stays namespaced under `thruput_sdk::xdr`.
pub use xdr::{
    ReadError, ReadResult, ReadXDR, Reader, WriteError, WriteResult, WriteXDR,
    Writer,
};

#[allow(unused_imports)]
pub use diagnostics::*;

#[doc = include_str!("../doc/macro-contract.md")]
pub use thruput_macros::contract;
// These must be enumerated to avoid stomping on the above docstring
pub use thruput_macros::ContractTypeSpec;
pub use thruput_macros::Event;
pub use thruput_macros::contract_exception;
pub use thruput_macros::contract_type;
pub use thruput_macros::exn;
pub use thruput_macros::export_interface_spec;
pub use thruput_macros::{
    hash_to_array, sha256_array_ident, sha256_ident, sha256_static, static_key,
};

pub use thruput_contract_spec::ContractTypeName;

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
/// thruput_sdk::storage_domain_uniqueness_test!();
/// ```
#[macro_export]
macro_rules! storage_domain_uniqueness_test {
    () => {
        #[cfg(all(test, not(target_arch = "riscv32")))]
        #[test]
        fn storage_domains_unique_across_linked_crates() {
            $crate::storage::assert_unique_storage_domains();
        }
    };
}

pub use thruput_macros::Deletable;

pub use thruput_macros::Storage;

#[doc = include_str!("../doc/macro-storage.md")]
pub use thruput_macros::storage;

// Mock contract execution environment

mod mock;

#[cfg(feature = "dev")]
pub use mock::*;
