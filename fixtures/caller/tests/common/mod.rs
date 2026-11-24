use caller::CallerClient;
use thruput_erc20_if::ERC20Client;
use thruput_sdk::*;

static ERC20_NONCE: DeployNonce =
    DeployNonce::new(thruput_sdk::sha256_static!("ERC20_TEST_NONCE"));
static CALLER_NONCE: DeployNonce =
    DeployNonce::new(thruput_sdk::sha256_static!("CALLER_TEST_NONCE"));

pub fn caller_test_inner(
    erc20_code: &CodeHash<ERC20Client>,
    code: &CodeHash<CallerClient>,
) {
    let erc20_addr = erc20_code.deploy(&ERC20_NONCE);
    let erc20_owner = get_current_address();

    let erc20 = erc20_addr.client();
    erc20.initialize(&erc20_owner);

    assert_eq!(0, erc20.total_supply());
    assert_eq!(0, erc20.get_balance());

    let deployed_addr = code.deploy(&CALLER_NONCE);

    let caller = deployed_addr.client();
    caller.initialize(&erc20_addr);

    let some_account = ContractAddress::zero();
    assert_eq!(0, caller.check_balance(&some_account));
}
