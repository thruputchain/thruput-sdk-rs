#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

mod implementation;
mod interface;

pub use interface::{Client, HTLC};
