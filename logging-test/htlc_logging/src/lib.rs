#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

mod interface;
mod implementation;

pub use interface::{Client, HTLC};
