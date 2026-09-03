/// Seals `StorageType` against casual hand-written impls.
///
/// `#[doc(hidden)] pub` rather than `pub(crate)` so the `#[storage]` derive
/// can implement it for the struct it generates, which lives in the user's
/// crate. Naming it directly is an explicit opt-out, not something reachable
/// by accident.
#[doc(hidden)]
pub trait Sealed {}
