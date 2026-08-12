#![allow(unused)]

mod ctx;
mod decimals;
mod etx;
mod help;
mod setup;
mod stats;
mod user;
mod util;

pub use ctx::*;
pub use decimals::*;
pub use etx::*;
pub use help::*;
pub use setup::*;
pub use stats::*;
pub use user::*;
pub use util::*;

pub use groundhog_erc20_if::ERC20Client;
#[cfg(feature = "metadata")]
pub use groundhog_erc20_metadata_if::*;
pub use sdk::*;
pub use swap::*;
pub use swap_driver::*;

// `sdk::U256` and this example's own `uint`-based `swap::U256` are both in
// scope via the globs above, so unqualified `U256` is ambiguous (E0659). This
// example still runs on its own type; name it explicitly to shadow both globs.
// The disambiguation goes away when the swap suite migrates to `sdk::U256`.
pub use swap::U256;
