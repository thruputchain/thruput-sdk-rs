use super::*;
pub use core::ops::Deref;

// Environment tracked in swap tests
#[derive(Clone)]
pub struct Etx {
    pub env: TestEnvironmentHandle,
    pub ctx: Ctx,
}

impl Etx {
    pub fn advance_block(&self) {
        self.env.advance_block();
    }

    pub fn exec_tx<F, T>(
        &self,
        addr: &ContractAddress,
        f: F,
    ) -> Result<T, TransactResult>
    where
        F: Fn() -> T + std::panic::RefUnwindSafe,
    {
        exec_tx(&self.env, addr, || f())
    }

    pub fn exec_user_tx<F, T>(
        &self,
        user: User,
        f: F,
    ) -> Result<T, TransactResult>
    where
        F: Fn(&User) -> T + std::panic::RefUnwindSafe,
    {
        exec_tx(&self.env, user.addr, || f(&user))
    }

    pub fn user_balances(&self, user: User) -> Result<(Amounts, Amount)> {
        let ctx = &self.ctx;
        self.exec_user_tx(user, |_| ctx.my_balances())
    }

    pub fn user_print_balances(&self, user: User) -> Result<()> {
        let ctx = &self.ctx;
        self.exec_user_tx(user, |_| ctx.user_print_balances(&user))
    }

    pub fn record_stats(
        &self,
        users: &[User],
    ) -> Result<SwapStats> {
        let ctx = &self.ctx;

        let get_user_stats = |&u| Ok((u, self.user_balances(u)?));
        let user_stats: UserStats = users
            .iter()
            .map(get_user_stats)
            .collect::<Result<UserStats>>()?;

        let pair_allowances: UserAllows = users
            .iter()
            .map(|&u| Ok((u, self.user_pledges(u)?)))
            .collect::<Result<UserAllows>>()?;

        Ok(SwapStats {
            user_stats,
            swap_pair: self.swap_pair_balances()?,
            pair_allowances,
        })
    }

    pub fn swap_pair_balances(&self) -> Result<(Amounts, Amount)> {
        let ctx = &self.ctx;
        exec_tx(&self.env, &TEST_ADDR, || {
            (
                [
                    ctx.token0().get_balance_of(&ctx.pair_addr),
                    ctx.token1().get_balance_of(&ctx.pair_addr),
                ],
                ctx.lp().get_balance_of(&ctx.pair_addr),
            )
        })
    }

    // What allowances has the user given to the pair contract?
    pub fn user_pledges(&self, user: User) -> Result<Amounts> {
        let ctx = &self.ctx;
        exec_tx(&self.env, &TEST_ADDR, || {
            [
                ctx.token0().allowance(&user.addr, &ctx.pair_addr),
                ctx.token1().allowance(&user.addr, &ctx.pair_addr),
            ]
        })
    }

    pub fn calculate_swap(&self, amounts_in: Amounts) -> Result<Amounts> {
        let ctx = &self.ctx;

        exec_tx(&self.env, &TEST_ADDR, || ctx.calculate_swap(amounts_in))
    }
}

impl Deref for Etx {
    type Target = Ctx;

    fn deref(&self) -> &Ctx {
        &self.ctx
    }
}
