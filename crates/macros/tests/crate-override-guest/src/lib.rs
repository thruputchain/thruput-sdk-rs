//! Guest-target half of the `#[contract_type_spec(crate = "...")]` coverage.
//!
//! The raw spec fragment the derive emits is behind `cfg(target_arch =
//! "riscv32")`, so the host test (`crates/macros/tests/crate_override.rs`,
//! which pins the derived fragment and its wire bytes) never compiles it.
//! Every other consumer of that code path reaches it through `::thruput_sdk`,
//! which the example and integration rv32 builds cover — but nothing else
//! builds a crate that resolves the macro's paths through a shim instead.
//!
//! This package exists to be built for rv32: `scripts/dev.sh` checks it under
//! `--target riscv32im-unknown-none-elf`, so a regression in the overridden
//! guest path fails there rather than in a downstream crate below
//! `thruput-sdk`.
//!
//! Kept deliberately minimal and in step with `crate_override.rs`: the type
//! mirrors `crates/u256`'s `U256`, the motivating consumer.

#![no_std]

use thruput_macros::ContractTypeSpec;

/// The shim a consuming crate provides. The derive only ever reaches for
/// `__contract_spec` and `__xdrs`, so matching those two names is the whole
/// contract.
#[doc(hidden)]
pub mod sdk_paths {
    pub use thruput_contract_spec as __contract_spec;
    pub use xdrs as __xdrs;
}

use thruput_contract_spec as spec;

const LIMBS: usize = 8;

// The field exists to give the derive something to describe; nothing reads it.
#[allow(dead_code)]
#[derive(ContractTypeSpec)]
#[contract_type_spec(crate = "crate::sdk_paths")]
pub struct Derived([u32; LIMBS]);

// The derive references `ContractTypeName` but never emits it, so a consumer
// supplies it.
impl spec::ContractTypeName for Derived {
    const NAME: spec::ContractSpecIdView<'static> = spec::ContractSpecIdView {
        crate_name: env!("CARGO_PKG_NAME"),
        major: "0",
        module_path: module_path!(),
        declared: "Derived",
    };
}
