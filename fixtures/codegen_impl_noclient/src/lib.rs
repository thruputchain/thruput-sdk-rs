#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;
pub use implementation::CalcEntrypoints;

pub use implementation::CalcClient;

pub use implementation::Calc;
