#![allow(clippy::manual_div_ceil)]

use sdk::{ReadXDR, WriteXDR};
use uint::construct_uint;

// U256 with 256 bits consisting of 4 x 64-bit words
construct_uint! {
    #[derive(ReadXDR, WriteXDR)]
    pub struct U256(4);
}
