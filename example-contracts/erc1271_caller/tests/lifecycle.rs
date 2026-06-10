use groundhog_erc1271_if::{ERC1271Client, ERC1271ClientError};
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

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let caller = erc1271_caller::CallerClient::new(&caller_addr);
            let result =
                caller.authorize_pair(&wallet_addr, &message, &message);
            assert_eq!(result, 3); // 1 + 2
        })
    })
    .assert_success();
}

#[test]
fn erc1271_single_token() {
    let (env, wallet_addr, caller_addr) = setup_env();

    // 8 bytes = 1 chunk = wallet produces 1 token
    let message: [u8; 8] = [42, 0, 0, 0, 0, 0, 0, 0]; // u64 = 42

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let caller = erc1271_caller::CallerClient::new(&caller_addr);
            let token =
                caller.authorize_single(&wallet_addr, &message, &message);
            let value: u64 = caller.consume_token(token);
            assert_eq!(value, 42);
        })
    })
    .assert_success();
}

#[test]
fn erc1271_rejection_throws_error() {
    let (env, wallet_addr, _caller_addr) = setup_env();

    let message: [u8; 8] = [1, 0, 0, 0, 0, 0, 0, 0];
    let bad_sig: [u8; 8] = [99, 0, 0, 0, 0, 0, 0, 0];

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let result = ERC1271Client::new(&wallet_addr)
                .try_is_valid_signature(&message, &bad_sig);
            match result {
                Err(ERC1271ClientError::ERC1271Error(e)) => {
                    assert_eq!(e as i32, 1); // InvalidSignature = 1
                }
                other => {
                    panic!(
                        "Expected Throw(InvalidSignature), got: {:?}",
                        other
                    )
                }
            }
        })
    })
    .assert_success();
}

#[test]
fn erc1271_authorize_none_empty_message() {
    let (env, wallet_addr, caller_addr) = setup_env();

    let message: [u8; 0] = [];

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let caller = erc1271_caller::CallerClient::new(&caller_addr);
            caller.authorize_none(&wallet_addr, &message, &message);
        })
    })
    .assert_success();
}
