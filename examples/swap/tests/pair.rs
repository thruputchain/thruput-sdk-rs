use erc20::interface::ERC20;
use erc20::interface::client::ERC20Client;
use sdk::prelude::*;
use swap::*;
use swap::{Result, Error};

#[test]
fn swap_test() {
    let mut env = sdk::TestEnvironment::new();

    // Register erc20 and instantiate for two tokens
    let erc20_contract = erc20::test_setup::register(&mut env);
    let token0_hash = env.setup_create_contract(&erc20_contract);
    let token1_hash = env.setup_create_contract(&erc20_contract);

    // Register and instantiate swap
    let swap_contract = test_setup::register(&mut env);
    let swap_hash = env.setup_create_contract(&swap_contract);

    let mut handler = env.prepare_test(move || -> i32 {
        match swap_test_inner(&swap_hash, &token0_hash, &token1_hash) {
            Ok(_) => 0,
            Err(error) => panic!("test failed: {:?}", error),
        }
    });

    let res = handler.root_invoke();
    assert_eq!(res.result.panic, sdk::InvokeErrorCode::kInvokeNone as i32);
}

fn swap_test_inner(
    swap_hash: &SHA256Hash,
    token0_hash: &SHA256Hash,
    token1_hash: &SHA256Hash,
    ) -> Result<()> {
    //
    // Deploy & initialize the swap-pair contract
    let nonce = SHA256Hash::from(ByteArray::from([7; 32]));
    let swap_addr = deploy(&nonce, swap_hash);
    let mut swap_pair = ISwapV2PairClient::new(&swap_addr);

    // The swap contract is the "owner" of the token contracts
    let token_owner = &swap_addr;

    // Deploy & initialize two token contracts
    let nonce0 = SHA256Hash::from(ByteArray::from([90; 32]));
    let nonce1 = SHA256Hash::from(ByteArray::from([91; 32]));
    let token0_addr = deploy(&nonce0, token0_hash);
    let token1_addr = deploy(&nonce1, token1_hash);
    let mut token0 = ERC20Client::new(&token0_addr);
    let mut token1 = ERC20Client::new(&token1_addr);
    ERC20::initialize(Some(&mut token0), token_owner).unwrap();
    ERC20::initialize(Some(&mut token1), token_owner).unwrap();

    let factory_addr = get_current_address().unwrap(); // whatever, for now

    ISwapV2Pair::initialize_pair(Some(&mut swap_pair), &factory_addr, &token0_addr, &token1_addr)?;

    let (r0, r1, _ts) = swap_pair.get_reserves()?;
    assert_eq!(r0, 0);
    assert_eq!(r1, 0);

    let liquidity = swap_pair.get_liquidity()?;
    assert_eq!(0, liquidity);

    swap_pair.sync()?;

    let somebody = ContractAddress::from([42; 32]);
    swap_pair.skim(&somebody)?;

    let lp_addr = ContractAddress::from([43; 32]);
    assert_eq!(swap_pair.mint_lp(&lp_addr), Err(Error::InsufficientLiquidityMinted));
    assert_eq!(swap_pair.burn_lp(&lp_addr), Err(Error::NoLiquidityToBurn));

    Ok(())
}

// below helpers should be somehow replaced by sdk features

fn deploy(nonce: &SHA256Hash, contract_hash: &SHA256Hash) -> ContractAddress {
    let addr = deploy_contract(nonce, contract_hash).unwrap_or_else(|e| {
        if let Ok(errcode) = TryInto::<SyscallErrorCode>::try_into(e) {
            panic!("deploy failed ({:?}) for {:x}", errcode, Hex(contract_hash));
        } else {
            panic!("deploy failed: {:?}", e);
        }
    });

    println!("DEPLOYED {:x} at 0x{:x}", Hex(contract_hash), Hex(&addr));

    addr
}

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
                for b in &slice[slice.len() - 4 ..] {
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
