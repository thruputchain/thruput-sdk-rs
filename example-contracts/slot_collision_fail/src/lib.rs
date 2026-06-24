#![cfg_attr(not(feature = "dev"), no_std)]

pub use sdk::storage::*;
pub use sdk::*;

/// Deliberate slot collision: both fields pinned to slot 3.
/// This must fail to compile with:
///   "storage slot 3 is assigned to more than one field"
#[storage]
pub struct Colliding {
    #[slot(3)]
    pub alpha: Value<u64>,
    #[slot(3)]  // deliberate collision
    pub beta: Value<u64>,
}
