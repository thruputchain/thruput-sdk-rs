//! Benchmark contract: one 256-bit operation, one implementation, per build.
//!
//! ## Why one build per measurement
//!
//! Both numbers this crate exists to produce are *marginal linkage* numbers, and
//! neither survives being measured in a binary that carries other operations:
//!
//! - **Codesize.** rv32 `.text` for an operation is what linking it adds. LLVM
//!   shares helpers across operations (`div` and `rem` are one routine; `sqrt`
//!   built on Newton iteration inlines a whole divide), so per-symbol
//!   attribution inside a combined binary double-counts some code and hides
//!   the rest. The only honest answer is the difference between a binary that
//!   calls the operation and one that does not -- which is what the `op-none`
//!   baseline build is for.
//! - **Gas.** Measuring one entrypoint of a many-entrypoint contract still
//!   charges the artifact's shared dispatch, and an artifact that links every
//!   operation has a different instruction cache and branch layout than the one
//!   a real contract would deploy.
//!
//! So `scripts/bigint-bench.sh` builds this package once per (backend,
//! operation) pair, each with exactly one `op-*` feature, and both metrics come
//! off the same artifact.
//!
//! ## Layout
//!
//! - [`backend`] -- the operation set, and one impl per implementation under
//!   test (`sdk::U256`, Parity's `uint`).
//! - [`ops`] -- the operations, generic over the backend.
//! - [`kernel`] -- the measured loop, and the `op-*` feature that selects it.
//!
//! Design note: `docs/superpowers/specs/2026-08-12-bigint-bench-design.md`.

#![no_std]

pub mod backend;
pub mod kernel;
pub mod ops;

mod contract;

/// The backend this build's kernel measures.
///
/// `backend-uint` (off by default) selects Parity's `uint`; with it off the
/// kernel measures `sdk::U256`. Enabling the feature does not remove the
/// `sdk::U256` backend -- both impls exist whenever their dependencies do,
/// which is what lets `tests/differential.rs` link them side by side. Only this
/// alias changes, and the build driver passes at most one backend feature.
#[cfg(feature = "backend-uint")]
pub type Selected = backend::parity::PU256;

/// The backend this build's kernel measures. See the `backend-uint` variant.
#[cfg(not(feature = "backend-uint"))]
pub type Selected = backend::u256::GroundhogU256;
