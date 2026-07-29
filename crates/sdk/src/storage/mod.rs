//! Datatypes backed by the key-value store.
//!
//! Note: Using lower-level syscall wrappers like `bytestring_set` can
//! interfere with this module's use of the database, resulting in
//! unexpected application errors.

pub mod error;
pub use error::*;

pub mod keypath;
pub use keypath::*;

mod sealed;

pub mod node;
pub use node::*;

pub mod deletable;
pub use deletable::*;

pub mod slot;
pub use slot::*;

pub mod bytestring;
pub use bytestring::*;

pub mod nnint;
pub use nnint::*;

pub mod balance;
pub use balance::*;

pub mod rcq;
pub use rcq::*;

pub mod value;
pub use value::*;

pub mod capacity_constraint;
pub use capacity_constraint::*;

pub mod map;
pub use map::*;

pub mod vector;
pub use vector::*;

#[cfg(not(any(target_family = "wasm", target_arch = "riscv32")))]
pub mod domain_registry;
#[cfg(not(any(target_family = "wasm", target_arch = "riscv32")))]
pub use domain_registry::*;

#[cfg(not(any(doc, target_family = "wasm", target_arch = "riscv32")))]
pub(crate) mod live_guards;

/// Guest builds have no live-guard tracker: `Drop` never runs past a
/// terminal Return there, which is the very hazard the native tracker
/// exists to catch. No-op stubs keep every call site free of `cfg`, and
/// compile to nothing — `LiveGuard`/`FrameGuardReset` are `Drop`-less ZSTs.
/// (Same dual-impl convention as [`crate::rt::trap`].)
#[cfg(any(doc, target_family = "wasm", target_arch = "riscv32"))]
pub(crate) mod live_guards {
    pub(crate) struct LiveGuard;
    impl LiveGuard {
        #[inline(always)]
        pub(crate) fn new() -> Self {
            Self
        }
    }

    pub(crate) struct FrameGuardReset;
    impl FrameGuardReset {
        #[inline(always)]
        pub(crate) fn new() -> Self {
            Self
        }
    }

    #[inline(always)]
    pub(crate) fn assert_no_live_guards() {}
}
