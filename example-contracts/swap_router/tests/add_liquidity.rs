mod common;
use common::*;

test_transaction! {
    fn alice_add_liquidity(etx: &Etx) -> Result<()> {
        let user = ALICE;

        // First, prime the AMM
        etx.prime()?;

        println!("{}", etx.record_stats(&[PRIMER, user])?);

        // Give an allowance of all our AB-tokens to the pair contract
        let (amounts_desired, _) = etx.user_balances(user)?;
        etx.user_allow_for_pair(user, amounts_desired)?;

        /* ----- */ etx.advance_block(); /* ----- */

        etx.user_add_liquidity(ALICE, amounts_desired)?;

        etx.process_outstanding_requests()?;

        /* ----- */ etx.advance_block(); /* ----- */

        println!("{}", etx.record_stats(&[PRIMER, ALICE])?);
        Ok(())
    }

    #[ignore]
    fn alice_add_some(etx: &Etx) -> Result<()> {
        let user = ALICE;

        // First, prime the AMM
        etx.prime()?;

        println!("{}", etx.record_stats(&[PRIMER, user])?);

        // Give an allowance of some of our AB-tokens to the pair contract
        let amt = 10*UNIT_AMT;
        etx.user_allow_for_pair(user, [amt, amt])?;

        /* ----- */ etx.advance_block(); /* ----- */

        etx.user_add_liquidity(ALICE, [amt, 0])?;

        etx.process_outstanding_requests()?;

        /* ----- */ etx.advance_block(); /* ----- */

        println!("{}", etx.record_stats(&[PRIMER, ALICE])?);
        Ok(())
    }

    fn primer_add_liquidity(etx: &Etx) -> Result<()> {
        etx.prime()
    }
}

impl Etx {
    #[rustfmt::skip]
    pub fn prime(&self) -> Result<()> {
        println!("{}", self.record_stats(&[PRIMER])?);

        // Give an allowance of all our AB-tokens to the pair contract
        let (amounts_desired, _) = self.user_balances(PRIMER)?;
        self.user_allow_for_pair(PRIMER, amounts_desired)?;

        /* ----- */ self.advance_block(); /* ----- */

        self.user_add_liquidity(PRIMER, amounts_desired)?;

        self.process_outstanding_requests()?;

        /* ----- */ self.advance_block(); /* ----- */

        println!("{}", self.record_stats(&[PRIMER])?);
        Ok(())
    }

    pub fn user_add_liquidity(&self, user: User, amounts_desired: Amounts) -> Result<()> {
        let ctx = &self.ctx;

        // Request to buy into the liquidity pool
        self.exec_user_tx(user, |_| {
            // Where the LP tokens go: back to the user
            let recipient = &get_current_address();

            // Unsure how to use these
            let amounts_min = [0, 0];

            ctx.router().add_liquidity(
                &ctx.token_addrs[0],
                &ctx.token_addrs[1],
                amounts_desired,
                amounts_min,
                recipient,
            );
        })
    }

    pub fn user_allow_for_pair(&self, user: User, amounts_allowed: Amounts) -> Result<()> {
        let ctx = &self.ctx;

        self.exec_user_tx(user, |_| {
            ctx.token0()
                .adjust_allowance(&ctx.pair_addr, amounts_allowed[0].try_into().unwrap());
            ctx.token1()
                .adjust_allowance(&ctx.pair_addr, amounts_allowed[1].try_into().unwrap());
        })
    }

    pub fn process_outstanding_requests(&self) -> Result<()> {
        let ctx = &self.ctx;

        let (n_processed, n_failed) =
            self.exec_tx(&TEST_ADDR, || ctx.driver().process_requests())?;
        assert_eq!(1, n_processed);
        assert_eq!(0, n_failed);

        println!("Processed {n_processed} requests");
        Ok(())
    }
}
