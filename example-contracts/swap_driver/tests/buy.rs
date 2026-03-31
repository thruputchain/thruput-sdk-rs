mod common;
use common::*;

// Serial buy

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

// Concurrent buy

#[test]
fn concurrent_buy_only() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;
        concurrent_buy(&etx)
    })
}

#[rustfmt::skip]
fn concurrent_buy(etx: &Etx) -> Result<()> {
    let ctx = &etx.ctx;

    // These are the funds that will be used to buy LP
    let (alice_balances, _) = etx.user_balances(ALICE)?;
    let (bob_balances, _) = etx.user_balances(BOB)?;

    // Record starting pair-reserve balances
    let (start_reserves, _) = etx.swap_pair_balances()?;

    /* ----- */ etx.advance_block(); /* ----- */

    // Alice buys liquidity tokens
    etx.exec_user_tx(ALICE, |u| ctx.user_buy_request(u))?;

    // Bob buys liquidity tokens
    etx.exec_user_tx(BOB, |u| ctx.user_buy_request(u))?;

    /* ----- */ etx.advance_block(); /* ----- */

    let (n_processed, n_failed) =
        etx.exec_tx(&TEST_ADDR, || ctx.driver().process_requests())?;
    assert_eq!(2, n_processed);
    assert_eq!(0, n_failed);

    println!("Processed {n_processed} requests");

    /* ----- */ etx.advance_block(); /* ----- */

    // Check changes to pair-reserve balances
    let (end_reserves, _) = etx.swap_pair_balances()?;

    let expected_reserves = balances_sum(start_reserves, alice_balances);
    let expected_reserves = balances_sum(expected_reserves, bob_balances);

    assert_eq!(end_reserves, expected_reserves);

    let deltas = balances_strict_sub(end_reserves, start_reserves);
    println!("Total buy (A, B): {:?}", deltas);

    etx.user_print_balances(ALICE)?;
    etx.user_print_balances(BOB)?;

    Ok(())
}

// Compare the results of serial and concurrent LP purchases
#[test]
fn compare_buy() {
    println!("\n[Serial]");
    let stats_serial = transact_assert_success(|| {
        let etx = setup_etx_primed()?;

        serial_buy(&etx)?;

        etx.record_stats(ALICE, BOB)
    });

    println!("\n[Concurrent]");
    let stats_concurrent = transact_assert_success(|| {
        let etx = setup_etx_primed()?;

        concurrent_buy(&etx)?;

        etx.record_stats(ALICE, BOB)
    });

    assert_eq!(stats_serial, stats_concurrent);
}
