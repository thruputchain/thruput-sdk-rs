mod common;
use common::*;

#[test]
fn serial_buy_only() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;
        serial_buy(&etx)
    });
}

#[rustfmt::skip]
fn serial_buy(etx: &Etx) -> Result<()> {
    let ctx = &etx.ctx;

    // Alice buys liquidity tokens
    etx.exec_user_tx(ALICE, |u| ctx.user_buy(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

    // Bob buys liquidity tokens
    etx.exec_user_tx(BOB, |u| ctx.user_buy(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

    Ok(())
}

// Serial buy+redeem

#[test]
#[rustfmt::skip]
fn serial_buy_redeem() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;
        let ctx = &etx.ctx;

        serial_buy(&etx)?;

        // Bob redeems liquidity tokens
        etx.exec_user_tx(BOB, |u| ctx.user_redeem(u))?;

        /* ----- */ etx.advance_block(); /* ----- */

        // Alice redeems liquidity tokens
        etx.exec_user_tx(ALICE, |u| ctx.user_redeem(u))?;

        /* ----- */ etx.advance_block(); /* ----- */

        // Do Alice and Bob have the same amount they started with?
        etx.exec_user_tx(ALICE, |u| {
            assert_eq!(u.init.0, ctx.token0().get_balance());
            assert_eq!(u.init.1, ctx.token1().get_balance());
        })?;
        etx.exec_user_tx(BOB, |u| {
            assert_eq!(u.init.0, ctx.token0().get_balance());
            assert_eq!(u.init.1, ctx.token1().get_balance());
        })?;

        Ok(())
    });
}
