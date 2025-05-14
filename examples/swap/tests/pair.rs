use swap::*;
use swap::Result;
use sdk::prelude::*;

#[test]
fn swap_test() {
    let mut env = sdk::TestEnvironment::new();

    let test_contract = test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    let mut handler = env.prepare_test(move || -> i32 {
        match swap_test_inner(&contract_hash) {
            Ok(_) => 0,
            Err(error) => panic!("test failed: {:?}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn swap_test_inner(contract_hash: &sdk::types::SHA256Hash) -> Result<()> {
    let nonce = SHA256Hash::from(ByteArray::from([7; 32]));
    let deployed_addr = deploy_contract(&nonce, contract_hash).unwrap();

    let mut pair = ISwapV2PairClient::new(&deployed_addr);

    ISwapV2Pair::initialize(Some(&mut pair))?;

    let (_r0, _r1, _ts) = pair.get_reserves();

    // pair.sync()?;

    Ok(())
}
