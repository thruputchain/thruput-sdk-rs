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

pub mod rcq;
pub use rcq::*;

pub mod value;
pub use value::*;

pub mod map;
pub use map::*;

pub mod vector;
pub use vector::*;

#[cfg(not(any(target_family = "wasm", target_arch = "riscv32")))]
pub mod domain_registry;
#[cfg(not(any(target_family = "wasm", target_arch = "riscv32")))]
pub use domain_registry::*;

#[cfg(all(
    not(any(doc, target_family = "wasm", target_arch = "riscv32")),
    feature = "std"
))]
pub(crate) mod live_guards;
