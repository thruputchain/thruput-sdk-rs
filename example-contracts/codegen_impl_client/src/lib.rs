#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;
pub use implementation::CalcProxy;
pub use implementation::exports::*;

pub use implementation::Calc;
