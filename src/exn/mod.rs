pub mod cross_exn;
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
    }

    impl<T> Exn for T where T: Debug + PartialEq + Error + WriteXDR + ReadXDROwned {}
}

#[cfg(not(feature = "debugging"))]
mod exn_impl {
    use crate::xdr::*;
    use core::fmt::Debug;

    pub trait Exn: Debug + PartialEq + WriteXDR + ReadXDROwned {}

    impl<T> Exn for T where T: Debug + PartialEq + WriteXDR + ReadXDROwned {}
}

pub use cross_exn::*;
pub use exn_impl::*;
pub use fault::*;
pub use library::*;
pub use wrapper::*;
