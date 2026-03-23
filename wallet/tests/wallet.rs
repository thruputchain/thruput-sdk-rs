use wallet::*;
use xdrs::*;
use groundhog_xdr::types::*;

// A few utilities that could be pulled out from the sdk
// if we make `wallet` independent of it
use sdk::{hash_to_array, sha256_static, hex_encode};

const WALLET_CONTRACT_HASH: SHA256Hash = sha256_static!("WALLET_CONTRACT_HASH");

#[test]
fn deploy() {
    let seed = Seed::new(hash_to_array!("my secret wallet passphrase"));

    let mut w = Wallet::new(seed);

    let account_name = "slushfund";

    w.create_account(account_name);

    let _stx = w.account_deploy_wallet(account_name, &WALLET_CONTRACT_HASH);
    let stx = w.account_deploy_wallet(account_name, &WALLET_CONTRACT_HASH);

    // These transactions should be unique
    assert_ne!(_stx, stx);

    let account = w.get_account(account_name).unwrap();

    println!("\nTransaction to deploy a wallet contract at address {}:\n\n{:#?}",
        hex_encode(&account.address()),
        stx,
    );

    let s = txs::encode(&stx).unwrap();
    println!("\nTXS ({} bytes):\n{}", s.len(), hex_encode(&s));

    let s = cxs::encode(&stx).unwrap();
    println!("\nCXS ({} bytes):\n{}", s.len(), hex_encode(&s));
}
