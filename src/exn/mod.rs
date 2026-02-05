mod cross_exn;
mod library;
mod fault;
mod wrapper;

use crate::xdr::*;
use core::fmt::Debug;
use core::error::Error;

pub trait Exn: Debug + PartialEq + Error + WriteXDR + ReadXDROwned {}

impl<T> Exn for T
where
    T: Debug + PartialEq + Error + WriteXDR + ReadXDROwned {}

pub use cross_exn::*;
pub use library::*;
pub use fault::*;
pub use wrapper::*;
