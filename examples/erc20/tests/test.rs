use erc20::implementation::test_setup;
use erc20_result_test::*;
use sdk::*;

#[test]
fn check() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = test_setup::register(&mut env);
    let test_contract_hash = env.setup_create_contract(&test_contract);
    let test_contract_addr =
        deploy_address(&OWNER, &NONCE, &test_contract_hash);

    env.setup_grant_storage_tokens(&test_contract_addr, 1_000_000_000);

    let mut handler = env.prepare_test(move || -> () {
        erc20_test(&test_contract_hash)
            .unwrap_or_else(|e| panic!("test failed: {e}"))
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);

    assert!(handler.try_commit_transaction());

    let event_log = handler.get_event_logs().unwrap();
    let events = &event_log.events;

    erc20_test_event_log_check(events);
}

// (General test utility)
fn deploy_address(
    deployer_address: &ContractAddress,
    nonce: &Hash256,
    contract_hash: &Hash256,
) -> ContractAddress {
    use groundhog_xdr::contract::ContractDeploymentHashPreimage;
    use sha2::{Digest, Sha256};

    let preimage = ContractDeploymentHashPreimage {
        deployer_address: *deployer_address,
        nonce: *nonce,
        contract_hash: *contract_hash,
    };
    let preimage: Vec<u8> = xdrs::serialize_txs::encode(&preimage).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(preimage.as_slice());

    let mut addr = [1u8; 32];
    hasher.finalize_into((&mut addr).into());

    addr.into()
}
