#![feature(maybe_uninit_slice)]

use sdk::prelude::*;
// use sdk::keyed::*;

#[derive(Clone, Copy, Debug, Default)]
struct Keyd;

#[cfg(feature = "guest")]
// Contract methods (below) exposed with C ABI for running as guest
pub use exports::*;

#[contract(exports = "exports", storage = "false")]
impl Keyd {
    pub fn hello(self) {
    }
}
