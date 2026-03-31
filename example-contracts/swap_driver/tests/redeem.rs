mod common;
use common::*;

// Serial buy+redeem

#[test]
#[rustfmt::skip]
fn serial_redeem() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;
        _serial_redeem(&etx)
    })
}
#[rustfmt::skip]
fn _serial_redeem(etx: &Etx) -> Result<()> {
        let ctx = &etx.ctx;

    // Alice buys liquidity tokens
    etx.exec_user_tx(ALICE, |u| ctx.user_buy(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

    // Bob buys liquidity tokens
    etx.exec_user_tx(BOB, |u| ctx.user_buy(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

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
}

#[test]
fn concurrent_redeem() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;
        _concurrent_redeem(&etx)
    })
}

#[rustfmt::skip]
fn _concurrent_redeem(etx: &Etx) -> Result<()> {
    let ctx = &etx.ctx;

    // Alice buys liquidity tokens
    etx.exec_user_tx(ALICE, |u| ctx.user_buy(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

    // Bob buys liquidity tokens
    etx.exec_user_tx(BOB, |u| ctx.user_buy(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

    // Record starting pair-reserve balances
    let (_, pair_start_lp) = etx.swap_pair_balances()?;
    assert_eq!(pair_start_lp, 0);

    // Record balances before redemption
    let (alice_start_balances, _) = etx.user_balances(ALICE)?;
    let (bob_start_balances, _) = etx.user_balances(BOB)?;

    // Bob allows his LP to the pair contract
    let bob_allowed = etx.exec_user_tx(BOB, |u| ctx.user_redeem_prepare(u))?;

    // Alice allows her LP to the pair contract
    let alice_allowed = etx.exec_user_tx(ALICE, |u| ctx.user_redeem_prepare(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

    // Bob requests redemption
    etx.exec_user_tx(BOB, |u| ctx.user_redeem_request(u, bob_allowed))?;

    // Bob requests redemption
    etx.exec_user_tx(ALICE, |u| ctx.user_redeem_request(u, alice_allowed))?;

    let (n_processed, n_failed) =
        etx.exec_tx(&TEST_ADDR, || ctx.driver().process_requests())?;
    assert_eq!(2, n_processed);
    assert_eq!(0, n_failed);

    println!("Processed {n_processed} requests");

    /* ----- */ etx.advance_block(); /* ----- */

    // Check changes

    let (_, pair_end_lp) = etx.swap_pair_balances()?;

    // The pair should have burned the redeemed LP
    assert_eq!(pair_end_lp, 0);

    let (alice_end_balances, alice_end_lp) = etx.user_balances(ALICE)?;
    let (bob_end_balances, bob_end_lp) = etx.user_balances(BOB)?;

    // Alice and Bob should have emptied their LP
    assert_eq!(alice_end_lp, 0);
    assert_eq!(bob_end_lp, 0);

    let alice_deltas = balances_strict_sub(alice_end_balances, alice_start_balances);
    let bob_deltas = balances_strict_sub(bob_end_balances, bob_start_balances);

    // Alice and Bob should both have gained A/B tokens in exchange for the
    // redemption.
    assert!(alice_deltas[0] > 0);
    assert!(alice_deltas[1] > 0);
    assert!(bob_deltas[0] > 0);
    assert!(bob_deltas[1] > 0);

    Ok(())
}

// Compare the results of serial and concurrent redemptions
#[test]
fn compare_redeem() {
    println!("\n[Serial]");
    let stats_serial = transact_assert_success(|| {
        let etx = setup_etx_primed()?;

        _serial_redeem(&etx)?;

        etx.record_stats(ALICE, BOB)
    });

    println!("\n[Concurrent]");
    let stats_concurrent = transact_assert_success(|| {
        let etx = setup_etx_primed()?;

        _concurrent_redeem(&etx)?;

        etx.record_stats(ALICE, BOB)
    });

    assert_eq!(stats_serial, stats_concurrent);
}
