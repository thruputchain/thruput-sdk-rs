mod cross_exn;
mod fault;
mod library;
mod wrapper;

use crate::xdr::*;
use core::error::Error;
use core::fmt::Debug;

pub trait Exn: Debug + PartialEq + Error + WriteXDR + ReadXDROwned {}

impl<T> Exn for T where T: Debug + PartialEq + Error + WriteXDR + ReadXDROwned {}

pub use cross_exn::*;
pub use fault::*;
pub use library::*;
pub use wrapper::*;
