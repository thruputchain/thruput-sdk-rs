use sdk::*;
use core::result::Result;
use groundhog_erc20_if::{ERC20, ERC20Error, Amount, Difference};
use erc20::ERC20Client;
use swap::{ISwapV2Pair, ISwapV2PairClient};
use swap::{erc20_test_setup, swap_test_setup};

#[test]
fn swap_test() {
    let mut env = sdk::TestEnvironment::no_cost();

    // Register erc20 and instantiate for two tokens
    let erc20_contract = erc20::test_setup::register(&mut env);
    let token0_hash = env.setup_create_contract(&erc20_contract);
    let token1_hash = env.setup_create_contract(&erc20_contract);

    // Register and instantiate swap
    let swap_contract = {
        let mut c = env.add_contract();

        // Register methods for both interfaces provided by the swap contract
        erc20_test_setup::register_mock(&mut c);
        swap_test_setup::register_mock(&mut c);
        c.as_contract()
    };
    let swap_hash = env.setup_create_contract(&swap_contract);

    sdk::contract_exec(env, || swap_test_inner(&swap_hash, &token0_hash, &token1_hash));
}

fn swap_test_inner(
    swap_hash: &SHA256Hash,
    token0_hash: &SHA256Hash,
    token1_hash: &SHA256Hash,
) {
    let test_addr = get_current_address();
    println!("Test address: {:x}", test_addr);

    // Deploy the swap-pair contract
    let nonce: SHA256Hash = [7; 32].into();
    let pair_addr = deploy(&nonce, swap_hash);
    let mut pair = ISwapV2PairClient::new(&pair_addr);
    let mut lp_token = ERC20Client::new(&pair_addr);

    // Deploy two token contracts
    let nonce0: SHA256Hash = [90; 32].into();
    let nonce1: SHA256Hash = [91; 32].into();
    let token0_addr = deploy(&nonce0, token0_hash);
    let token1_addr = deploy(&nonce1, token1_hash);
    let mut token0 = ERC20Client::new(&token0_addr);
    let mut token1 = ERC20Client::new(&token1_addr);

    // Initialize the token contracts, putting them under our control
    let token_owner = &test_addr;
    token0.initialize(&token_owner);
    token1.initialize(&token_owner);

    // Initialize the swap-pair contract with the two tokens
    // (This also initializes the embedded ERC20 implementation.)
    let factory_addr = &test_addr;
    ISwapV2Pair::initialize(
        &mut pair,
        &factory_addr,
        &token0_addr,
        &token1_addr,
    );

    // Check how the initial state behaves

    let (r0, r1, _ts) = pair.get_reserves();
    assert_eq!(r0, 0);
    assert_eq!(r1, 0);

    let liquidity = pair.get_liquidity();
    assert_eq!(0, liquidity);

    pair.sync();

    let somebody = ContractAddress::from([42; 32]);
    pair.skim(&somebody);

    let lp_addr = ContractAddress::from([43; 32]);
    assert!(pair.try_mint_lp(&lp_addr).is_err());
        // SwapError::InsufficientLiquidityMinted

    assert!(pair.try_burn_lp(&lp_addr).is_err());
        // SwapError::NoLiquidityToBurn

    // Create some tokens for swapping, giving them to alice
    let alice = &test_addr; // this test contract
    token0.mint(alice, 1_000_000_000);
    token1.mint(alice, 2_000_000_000);

    println!("token0 total_supply = {}", token0.total_supply());
    println!("token1 total_supply = {}", token1.total_supply());

    // As alice, provide liquidity:
    {
        // Transfer some tokens into the pair
        token0.transfer(&pair_addr, 1_000_000);
        token1.transfer(&pair_addr, 2_000_000);

        // Mint LP tokens to alice in exchange for the tokens
        let amount_minted = pair.mint_lp(alice);

        println!("Minted LP tokens for alice: {}", amount_minted);

        println!("LP token total_supply: {}", pair.get_liquidity());

        // We can interrogate the embedded ERC20 implementation either
        // by a direct contract call, or via the swap contract
        assert_eq!(lp_token.total_supply(), pair.get_liquidity());
    }

    // As alice, retrieve pair tokens:
    {
        // TODO: check that these numbers (from a trial run) are actually right

        let lp_amount = lp_token.get_balance();
        println!("Alice has {} LP tokens", lp_amount);

        assert_eq!(lp_token.get_balance(), 1413213);

        // Return LP tokens to pair contract
        lp_token.transfer(&pair_addr, lp_amount);

        // Alice should have zero LP tokens now
        assert_eq!(lp_token.get_balance(), 0);

        // Burn the LP tokens in exchange for pair tokens
        pair.burn_lp(alice);

        // All liquidity has been drained, except a retained fee
        assert_eq!(pair.get_liquidity(), 1000);

        println!(
            "Alice has ({}, {}) of (token0, token1)",
            token0.get_balance(),
            token1.get_balance(),
        );

        // Alice should now have what she started with, minus
        // transaction fees
        assert_eq!(token0.get_balance(), 999999292);
        assert_eq!(token1.get_balance(), 1999998585);
    }

    // TODO: check swap()
    // TODO: check other contract error paths
}

// below helpers should be somehow replaced by sdk features

mod hex {
    use core::fmt;

    pub struct Hex<'a, T>(pub &'a T);

    impl<'a, T> fmt::LowerHex for Hex<'a, T>
    where
        T: AsRef<[u8]>,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let slice: &[u8] = self.0.as_ref();
            if slice.len() > 16 {
                for b in &slice[0..4] {
                    write!(f, "{:02x}", b)?;
                }
                write!(f, "...")?;
                for b in &slice[slice.len() - 4..] {
                    write!(f, "{:02x}", b)?;
                }
            } else {
                for b in slice {
                    write!(f, "{:02x}", b)?;
                }
            }
            Ok(())
        }
    }
}
use hex::*;
