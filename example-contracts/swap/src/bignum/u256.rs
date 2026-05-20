#![allow(clippy::manual_div_ceil)]

// U256 with 256 bits consisting of 4 x 64-bit words
uint::construct_uint! {
    #[sdk::contract_type]
    pub struct U256(4);
}
