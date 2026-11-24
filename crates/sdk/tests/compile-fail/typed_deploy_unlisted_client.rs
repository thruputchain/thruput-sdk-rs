#![allow(unexpected_cfgs)]

//! A deployment gives only the clients its code hash lists.

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
    let hash = SHA256Hash::new([1; 32]);
    let nonce = DeployNonce::new(hash);
    let token = CodeHash::<TokenClient>::new(hash);
    let both = CodeHash::<(TokenClient, MetadataClient)>::new(hash);

    let _ = MetadataClient::at(&token.deploy(&nonce));
    let _ = OtherClient::at(&both.deploy_address(&nonce));
    let _: OtherClient = token.deploy(&nonce).client();
    let _: Address<OtherClient> = token.deploy(&nonce);
}
