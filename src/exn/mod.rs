pub(crate) mod cross_exn;
mod fault;
mod library;
mod wrapper;

#[cfg(feature = "debugging")]
mod exn_impl {
    use crate::xdr::*;
    use core::error::Error;
    use core::fmt::Debug;

    pub trait Exn:
        Debug + PartialEq + Error + WriteXDR + ReadXDROwned
    {
        const SELECTOR: u64;
    }
}

#[cfg(not(feature = "debugging"))]
mod exn_impl {
    use crate::xdr::*;
    use core::fmt::Debug;

    pub trait Exn: Debug + PartialEq + WriteXDR + ReadXDROwned {
        const SELECTOR: u64;
    }
}

/// Exn impl for Infallible — this type can never be instantiated,
/// so its selector is 0 (a reserved/unused value).
impl Exn for core::convert::Infallible {
    const SELECTOR: u64 = 0;
}

mod raw_error;
pub use raw_error::*;

pub use cross_exn::*;
pub use exn_impl::*;
pub use fault::*;
pub use library::*;
pub use wrapper::*;
