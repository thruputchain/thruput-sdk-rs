use sdk::*;

fn setup_env() -> (sdk::TestEnvironmentHandle, ContractAddress, ContractAddress)
{
    let env = sdk::TestEnvironment::no_cost();

    let wallet_mock = erc1271_wallet::register_mock_contract(&env);
    let caller_mock = erc1271_caller::register_mock_contract(&env);

    let wallet_addr: ContractAddress = [0xAA; 32].into();
    let caller_addr: ContractAddress = [0xBB; 32].into();

    wallet_mock.deploy_contract_to(&wallet_addr);
    caller_mock.deploy_contract_to(&caller_addr);

    (env, wallet_addr, caller_addr)
}

#[test]
fn erc1271_two_token_lifecycle() {
    let (env, wallet_addr, caller_addr) = setup_env();

    // 16 bytes = 2 chunks of 8 = wallet produces 2 tokens
    let message: [u8; 16] = [
        1, 0, 0, 0, 0, 0, 0, 0, // u64 = 1
        2, 0, 0, 0, 0, 0, 0, 0, // u64 = 2
    ];

    sdk::contract_exec(env, move || {
        let caller = erc1271_caller::CallerClient::new(&caller_addr);
        let result = caller.authorize_pair(&wallet_addr, &message, &message);
        assert_eq!(result, 3); // 1 + 2
    })
    .unwrap_or_else(|e| panic!("ERC1271 lifecycle test failed: {:?}", e));
}

#[test]
fn erc1271_single_token() {
    let (env, wallet_addr, caller_addr) = setup_env();

    // 8 bytes = 1 chunk = wallet produces 1 token
    let message: [u8; 8] = [42, 0, 0, 0, 0, 0, 0, 0]; // u64 = 42

    sdk::contract_exec(env, move || {
        let caller = erc1271_caller::CallerClient::new(&caller_addr);
        let token = caller.authorize_single(&wallet_addr, &message, &message);
        let value: u64 = caller.consume_token(token);
        assert_eq!(value, 42);
    })
    .unwrap_or_else(|e| panic!("ERC1271 single token test failed: {:?}", e));
}
