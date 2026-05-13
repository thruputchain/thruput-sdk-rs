mod common;
use common::*;

#[rustfmt::skip]
#[test]
fn primer_add_liquidity() {
    transact_assert_success(|| {
        let etx = setup_etx()?;
        _primer_add_liquidity(&etx)
    });
}

fn _primer_add_liquidity(etx: &Etx) -> Result<()> {
    let ctx = &etx.ctx;

    println!("{}", etx.record_stats(&[PRIMER])?);

    let amounts_min = [0, 0];

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

    // Request to buy into the liquidity pool
    etx.exec_user_tx(PRIMER, |_| {
        // Where the LP tokens go
        let recipient = &get_current_address();

        ctx.router().add_liquidity(
            &ctx.token_addrs[0],
            &ctx.token_addrs[1],
            amounts_desired,
            amounts_min,
            recipient,
        );
    })?;

    // Process outstanding requests
    let (n_processed, n_failed) =
        etx.exec_tx(&TEST_ADDR, || ctx.driver().process_requests())?;
    assert_eq!(1, n_processed);
    assert_eq!(0, n_failed);

    println!("Processed {n_processed} requests");

    /* ----- */ etx.advance_block(); /* ----- */

    println!("{}", etx.record_stats(&[PRIMER])?);

    Ok(())
}
