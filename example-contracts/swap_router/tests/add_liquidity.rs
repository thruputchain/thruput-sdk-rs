mod common;
use common::*;

test_transaction! {
    fn alice_add_liquidity(etx: &Etx) -> Result<()> {
        let ctx = &etx.ctx;

        // First, prime the AMM
        prime(etx)?;

        println!("{}", etx.record_stats(&[PRIMER, ALICE])?);

        // Give an allowance of all our AB-tokens to the pair contract
        // TODO: Get quote, then add_liquidity.
        //
        let amounts_desired = etx.exec_user_tx(ALICE, |_| {
            let amounts_desired = [ctx.token0().get_balance(), ctx.token1().get_balance()];

            ctx.token0()
                .adjust_allowance(&ctx.pair_addr, amounts_desired[0].try_into().unwrap());
            ctx.token1()
                .adjust_allowance(&ctx.pair_addr, amounts_desired[1].try_into().unwrap());

            amounts_desired
        })?;

        /* ----- */ etx.advance_block(); /* ----- */

        user_add_liquidity(etx, ALICE, amounts_desired)?;

        process_outstanding_requests(etx)?;

        /* ----- */ etx.advance_block(); /* ----- */

        println!("{}", etx.record_stats(&[PRIMER, ALICE])?);
        Ok(())
    }
}

#[rustfmt::skip]
#[test]
fn primer_add_liquidity() {
    transact_assert_success(|| {
        let etx = setup_etx()?;
        prime(&etx)
    });
}

fn prime(etx: &Etx) -> Result<()> {
    let ctx = &etx.ctx;

    println!("{}", etx.record_stats(&[PRIMER])?);

    // Give an allowance of all our AB-tokens to the pair contract
    let amounts_desired = etx.exec_user_tx(PRIMER, |_| {
        let amounts_desired = [ctx.token0().get_balance(), ctx.token1().get_balance()];

        ctx.token0()
            .adjust_allowance(&ctx.pair_addr, amounts_desired[0].try_into().unwrap());
        ctx.token1()
            .adjust_allowance(&ctx.pair_addr, amounts_desired[1].try_into().unwrap());

        amounts_desired
    })?;

    /* ----- */ etx.advance_block(); /* ----- */

    user_add_liquidity(etx, PRIMER, amounts_desired);

    process_outstanding_requests(etx)?;

    /* ----- */ etx.advance_block(); /* ----- */

    println!("{}", etx.record_stats(&[PRIMER])?);
    Ok(())
}

fn user_add_liquidity(etx: &Etx, user: User, amounts_desired: Amounts) -> Result<()> {
    let ctx = &etx.ctx;

    // Request to buy into the liquidity pool
    etx.exec_user_tx(user, |_| {
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

fn process_outstanding_requests(etx: &Etx) -> Result<()> {
    let ctx = &etx.ctx;

    let (n_processed, n_failed) =
        etx.exec_tx(&TEST_ADDR, || ctx.driver().process_requests())?;
    assert_eq!(1, n_processed);
    assert_eq!(0, n_failed);

    println!("Processed {n_processed} requests");
    Ok(())
}
