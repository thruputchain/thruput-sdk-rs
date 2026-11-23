#![allow(unexpected_cfgs)]

//! A typed address gives only the clients it lists.

use thruput_sdk::*;

#[contract(version = "1.0.0")]
pub trait Token {
    fn label(&self) -> u32;
}

#[contract(version = "1.0.0")]
pub trait Metadata {
    fn name_len(&self) -> u32;
}

#[contract(version = "1.0.0")]
pub trait Other {
    fn other(&self) -> u32;
}

fn main() {
    let address = ContractAddress::new([1; 32]);
    let token = Address::<TokenClient>::new(address);
    let both = Address::<(TokenClient, MetadataClient)>::new(address);

    let _ = MetadataClient::at(&token);
    let _ = OtherClient::at(&both);
    let _: Address<OtherClient> = both.narrow();
}
