#![allow(unexpected_cfgs)]

//! A deploy nonce and a code hash do not stand in for each other, or for a
//! bare hash.

use thruput_sdk::*;

#[contract(version = "1.0.0")]
pub trait Token {
    fn label(&self) -> u32;
}

fn main() {
    let hash = SHA256Hash::new([1; 32]);
    let deployer = ContractAddress::new([2; 32]);
    let code = CodeHash::<TokenClient>::new(hash);
    let nonce = DeployNonce::new(hash);

    // A code hash where the nonce goes.
    let _ = code.deploy(&code);
    let _ = code.deploy_address(&code);
    let _ = code.deploy_address_for(&deployer, &code);

    // The two swapped: a nonce does not deploy.
    let _ = nonce.deploy(&code);

    // A bare hash where the nonce goes.
    let _ = code.deploy(&hash);

    // Neither passes as a bare hash, in either position.
    let _ = deploy_contract(&code, &nonce);
    let _ = deploy_contract(&nonce, &code);
}
