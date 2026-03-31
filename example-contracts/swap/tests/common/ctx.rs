use super::*;

pub type Addresses = [ContractAddress; 2];

// Contract context tracked in swap tests
#[derive(Copy, Clone)]
pub struct Ctx {
    pub pair_addr: ContractAddress,
    pub liquidity_token: ContractAddress,
    pub token_addrs: Addresses,
}

impl Ctx {
    pub fn token0(&self) -> ERC20Client {
        ERC20Client::new(&self.token_addrs[0])
    }
    pub fn token1(&self) -> ERC20Client {
        ERC20Client::new(&self.token_addrs[1])
    }
    pub fn pair(&self) -> ISwapV2PairClient {
        ISwapV2PairClient::new(&self.pair_addr)
    }
    pub fn lp(&self) -> ERC20Client {
        ERC20Client::new(&self.liquidity_token)
    }
}

impl Ctx {
    pub fn my_balances(&self) -> (Amounts, Amount) {
        let token_balances =
            [self.token0().get_balance(), self.token1().get_balance()];
        let lp_balance = self.lp().get_balance();

        (token_balances, lp_balance)
    }

    pub fn user_print_balances(&self, user: &User) {
        assert_eq!(&get_current_address(), user.addr);

        print_balances(user.name, self.my_balances());
    }

    // As a user, provide AB-token liquidity in exchange for LP tokens
    pub fn user_buy(&self, user: &User) {
        // user_print_balances(ctx, user);

        let amt0 = self.token0().get_balance();
        let amt1 = self.token1().get_balance();

        // Transfer our AB-tokens to the pair contract
        self.token0()
            .transfer(&self.pair_addr, amt0.try_into().unwrap());
        self.token1()
            .transfer(&self.pair_addr, amt1.try_into().unwrap());

        let lp_minted = self.pair().mint_lp(user.addr);

        // My accounts should be drained
        assert_eq!(0, self.token0().get_balance());
        assert_eq!(0, self.token1().get_balance());

        {
            println!(
                "> {} bought {:.3} LP",
                user.name,
                decimalized(lp_minted)
            );
            self.user_print_balances(user);
        }
    }

    // As a user, redeem swap-pair tokens
    pub fn user_redeem(&self, user: &User) {
        let pair = self.pair();
        let lp = self.lp();

        // Return all my LP tokens to the swap-pair contract
        let return_amt = lp.get_balance();
        lp.transfer(&self.pair_addr, return_amt);
        println!("> {} redeemed {:.3} LP", user.name, decimalized(return_amt));

        // I should have zero LP tokens now
        assert_eq!(lp.get_balance(), 0);

        // Redeem the LP tokens for pair tokens
        pair.burn_lp(user.addr);

        self.user_print_balances(user);
    }

    // Make sure we're not holding any LP tokens
    pub fn factory_sync_skim_pair(&self) {
        let pair = self.pair();

        let (r0, r1, _ts) = pair.get_reserves();
        assert_eq!(r0, 0);
        assert_eq!(r1, 0);

        let our_lp = pair.get_liquidity();
        assert_eq!(0, our_lp);

        // This should be a no-op
        pair.sync();

        hash_ident! { static SOMEBODY: ContractAddress; }

        // This should be a no-op
        pair.skim(&SOMEBODY);

        assert!(
            pair.try_mint_lp(&SOMEBODY)
                .is_err_and(|e| is_pair_error_and(e, |e| e
                    == ISwapV2PairError::InsufficientLiquidityMinted))
        );
    }
}
