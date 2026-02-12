use groundhog_erc20_if::ERC20Client;
use caller::CallerClient;
use sdk::*;

static ERC20_NONCE: SHA256Hash = sdk::sha256_static!("ERC20_TEST_NONCE");
static CALLER_NONCE: SHA256Hash = sdk::sha256_static!("CALLER_TEST_NONCE");

pub fn caller_test_inner(
    erc20_hash: &SHA256Hash,
    contract_hash: &SHA256Hash,
) {
    let erc20_addr = deploy_contract(&ERC20_NONCE, erc20_hash);
    let erc20_owner = get_current_address();

    let mut erc20 = ERC20Client::new(&erc20_addr);
    erc20.initialize(&erc20_owner);

    assert_eq!(0, erc20.total_supply());
    assert_eq!(0, erc20.get_balance());

    let deployed_addr = deploy_contract(&CALLER_NONCE, contract_hash);

    let mut caller = CallerClient::new(&deployed_addr);
    caller.initialize(&erc20_addr);

    let some_account = ContractAddress::zero();
    assert_eq!(0, caller.check_balance(&some_account));
}
