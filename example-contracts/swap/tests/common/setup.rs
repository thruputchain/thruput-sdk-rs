use super::*;

#[rustfmt::skip]
pub fn setup_etx() -> Result<Etx> {
    let env = sdk::TestEnvironment::no_cost();

    // Register the token contract
    let token_contract = erc20::register_mock_contract(&env);
    let token_hash = token_contract.as_hash().clone();

    // Register the swap-pair contract
    let pair_contract = swap::register_mock_contract(&env);
    let pair_hash = pair_contract.as_hash().clone();

    // Deploy and init the A/B token contracts
    let token_addrs = exec_tx(&env, &TEST_ADDR, || ab_tokens_deploy_init(&token_hash))?;

    /* ----- */ env.advance_block(); /* ----- */

    // Seed our users with A/B tokens
    exec_tx(&env, &TEST_ADDR, || airdrop_tokens(&token_addrs));

    /* ----- */ env.advance_block(); /* ----- */

    // As the factory, deploy and init a swap-pair contract
    let (pair_addr, liquidity_token) = exec_tx(&env, &FACTORY_ADDR, || {
        swap_pair_deploy_init(&pair_hash, &token_hash, token_addrs)
    })?;

    /* ----- */ env.advance_block(); /* ----- */

    let env = env.clone();
    let ctx = Ctx {
        pair_addr,
        liquidity_token,
        token_addrs
    };

    Ok(Etx { env, ctx })
}

#[rustfmt::skip]
pub fn setup_etx_primed() -> Result<Etx> {
    let etx = setup_etx()?;

    // Primer buys liquidity tokens, establishing a stable price
    etx.exec_user_tx(PRIMER, |u| etx.ctx.user_buy(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

    Ok(etx)
}

pub use setup_etx_primed as setup_pair_primed;

// Contract code

// Deploy and init A/B token contracts, given the hash of a registered token
// contract.
fn ab_tokens_deploy_init(token_hash: &SHA256Hash) -> Addresses {
    assert_eq!(&get_current_address(), &TEST_ADDR);

    // Deploy contracts for two tokens that we'd like to swap between
    hash_ident! { static TOKEN_0_NONCE: SHA256Hash; }
    hash_ident! { static TOKEN_1_NONCE: SHA256Hash; }
    let token0_addr = deploy_contract(&TOKEN_0_NONCE, token_hash);
    let token1_addr = deploy_contract(&TOKEN_1_NONCE, token_hash);

    let token0 = ERC20Client::new(&token0_addr);
    let token1 = ERC20Client::new(&token1_addr);

    // Initialize the token contracts, putting them under control of the test
    token0.initialize(TEST.addr);
    token1.initialize(TEST.addr);

    [token0_addr, token1_addr]
}

fn airdrop_tokens(token_addrs: &Addresses) {
    let token0 = ERC20Client::new(&token_addrs[0]);
    let token1 = ERC20Client::new(&token_addrs[1]);

    assert_eq!(token0.get_balance_of(ALICE.addr), 0);
    assert_eq!(token1.get_balance_of(ALICE.addr), 0);

    // Initial airdrops
    for u in USERS {
        token0.mint(u.addr, u.init.0);
        token1.mint(u.addr, u.init.1);
    }
}

// Deploy and init a swap-pair contract (which itself deploys an LP-token
// contract), given hashes of a registered pair contract and a registered
// token contract.
fn swap_pair_deploy_init(
    pair_hash: &SHA256Hash,
    token_hash: &SHA256Hash,
    token_addrs: Addresses,
) -> (ContractAddress, ContractAddress) {
    assert_eq!(&get_current_address(), &FACTORY_ADDR);

    // Deploy a new swap-pair contract
    hash_ident! { static PAIR_NONCE: SHA256Hash; }
    let pair_addr = deploy_contract(&PAIR_NONCE, pair_hash);

    let pair = ISwapV2PairClient::new(&pair_addr);

    // Initialize the pair contract with the two tokens
    // (This also initializes its embedded ERC20 implementation.)
    pair.initialize(
        &FACTORY_ADDR,
        &token_addrs[0],
        &token_addrs[1],
        token_hash,
    );

    let liquidity_token = pair.liquidity_token();

    (pair_addr, liquidity_token)
}
