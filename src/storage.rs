use crate::result::Result;

#[macro_use]
mod macros;

mod keypath;
pub use keypath::*;

mod load;
pub use load::*;

mod store;
pub use store::*;

mod keypath_seed;
pub use keypath_seed::*;

// Builtin storage types ...

mod bytestring;
pub use bytestring::*;

mod nnint;
pub use nnint::*;

mod handle;
pub use handle::*;

mod map;
pub use map::*;

mod vector;
pub use vector::*;
