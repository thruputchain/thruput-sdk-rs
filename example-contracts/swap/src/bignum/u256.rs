#![allow(clippy::manual_div_ceil)]

use uint::construct_uint;
use sdk::{ReadXDR, WriteXDR};

// U256 with 256 bits consisting of 4 x 64-bit words
construct_uint! {
    #[derive(ReadXDR, WriteXDR)]
    pub struct U256(4);
}
