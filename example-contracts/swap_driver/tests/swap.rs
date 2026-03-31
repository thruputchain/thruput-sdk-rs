mod common;
use common::*;
use std::iter::zip;

const SWAP_IN_AMT: Amount = 2 * UNIT_AMT;

/////////////////////// SERIAL /////////////////////////////

#[test]
fn serial_swap_both() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;
        serial_swap(&etx)
    });
}

#[test]
fn serial_swap_one() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;

        let user = CAROL;

        etx.user_print_balances(user)?;

        let amounts_in = [2 * UNIT_AMT, 0];
        let amounts_out = etx.calculate_swap(amounts_in)?;

        serial_swap_user(&etx, user, amounts_in, amounts_out)?;

        Ok(())
    });
}

fn serial_swap(etx: &Etx) -> Result<()> {
    etx.user_print_balances(CAROL)?;
    etx.user_print_balances(DAVE)?;

    // Calculate amounts_out ahead of time, to make this behave the same as the
    // concurrent_swap

    let carol_amounts_in = [2 * UNIT_AMT, 0];
    let carol_amounts_out = etx.calculate_swap(carol_amounts_in)?;

    let dave_amounts_in = [0, 2 * UNIT_AMT];
    let dave_amounts_out = etx.calculate_swap(dave_amounts_in)?;

    // Carol does a swap
    let user = CAROL;
    serial_swap_user(etx, user, carol_amounts_in, carol_amounts_out)?;

    // Dave swaps in the other direction
    let user = DAVE;
    serial_swap_user(etx, user, dave_amounts_in, dave_amounts_out)?;

    Ok(())
}

#[rustfmt::skip]
fn serial_swap_user(
    etx: &Etx,
    user: User,
    amounts_in: Amounts,
    amounts_out: Amounts,
) -> Result<()> {
    let ctx = &etx.ctx;

    // Send the purchasing funds to the swap-pair contract
    etx.exec_user_tx(user, |_| {
        ctx.token0().transfer(&ctx.pair_addr, amounts_in[0])
    })?;
    etx.exec_user_tx(user, |_| {
        ctx.token1().transfer(&ctx.pair_addr, amounts_in[1])
    })?;

    // let amounts_out = etx.calculate_swap(amounts_in)?;

    /* ----- */ etx.advance_block(); /* ----- */

    // Perform the swap
    etx.exec_user_tx(user, |user| {
        ctx.pair().swap(amounts_out[0], amounts_out[1], &user.addr)
    })?;

    println!("> {} swapped {:.3?} => {:.3?}", user.name,
        amounts_decimalized(amounts_in),
        amounts_decimalized(amounts_out),
    );

    /* ----- */ etx.advance_block(); /* ----- */

    etx.user_print_balances(user)?;

    Ok(())
}

/////////////////////// CONCURRENT /////////////////////////////

#[test]
fn concurrent_swap_one() {
    let user = CAROL;

    transact_assert_success(|| {
        let etx = setup_etx_primed()?;
        let strategy = [(user, [SWAP_IN_AMT, 0], None)];
        concurrent_swap_strategy(&etx, strategy)
    });
}

#[test]
fn concurrent_swap_only() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;
        concurrent_swap(&etx)
    });
}

fn concurrent_swap(etx: &Etx) -> Result<()> {
    // The two users involved, and their swap-in amounts
    let strategy = [
        (CAROL, [SWAP_IN_AMT, 0], None),
        (DAVE, [0, SWAP_IN_AMT], None),
    ];
    concurrent_swap_strategy(etx, strategy)
}

// Can we continue handling swap requests after one failed due to insufficient
// funds?
#[test]
fn concurrent_swap_transfer_failure() {
    transact_assert_success(|| {
        let etx = setup_etx_primed()?;

        // Note: Dave doesn't submit any funds but asks for something back from the
        // swap
        let carol_amts_in = [SWAP_IN_AMT, 0];
        let dave_amts_in = [0, 0];
        let dave_amts_out = etx.calculate_swap(carol_amts_in)?;
        let strategy = [
            (CAROL, carol_amts_in, None),
            (DAVE, dave_amts_in, Some(dave_amts_out)),
        ];

        concurrent_swap_strategy(&etx, strategy)
    });
}

#[rustfmt::skip]
fn concurrent_swap_strategy<const N: usize>(
    etx: &Etx,
    strategy: [(User, Amounts, Option<Amounts>); N],
) -> Result<()> {
    let ctx = &etx.ctx;

    let start_balances = strategy.map(|(user, _, _)| {
        etx.user_balances(user).unwrap()
    });

    for (user, _, _) in strategy {
        etx.user_print_balances(user)?
    }

    // Give the swap-pair contract access to the purchasing funds
    for (user, amounts_in, _) in strategy {
        println!("{} pledges {:.3?}", user.name, amounts_decimalized(amounts_in));
        etx.exec_user_tx(user, |_| {
            ctx.token0().adjust_allowance(
                &ctx.pair_addr,
                amounts_in[0].try_into().unwrap(),
            );
            ctx.token1().adjust_allowance(
                &ctx.pair_addr,
                amounts_in[1].try_into().unwrap(),
            );
        })?;
    }

    let strategy = strategy.map(|(user, amounts_in, amounts_out_opt)| {
        // Calculate what we want to get out of the swap from what we put in
        // (unless specific out-amounts have been specified in the strategy).
        let amounts_out = amounts_out_opt.unwrap_or_else(|| etx.calculate_swap(amounts_in).unwrap());

        (user, amounts_in, amounts_out)
    });

    /* ----- */ etx.advance_block(); /* ----- */

    for (user, _, _) in strategy {
        println!("{} has pledged {:.3?}", user.name, etx.user_pledges(user)?);
    }

    // Both users submit swap requests
    for (user, amounts_in, amounts_out) in strategy {
        etx.exec_user_tx(user, |_| {
            ctx.driver().request_swap(&user.addr, amounts_out)
        })?;

        println!("> {} requested swap {:.3?} => {:.3?}", user.name,
            amounts_decimalized(amounts_in),
            amounts_decimalized(amounts_out),
        );
    }

    // (No block boundary needed here.)

    // Process requests
    let (n_processed, _n_failed) =
        etx.exec_tx(&TEST_ADDR, || ctx.driver().process_requests())?;

    assert_eq!(n_processed, N as u32);

    /* ----- */ etx.advance_block(); /* ----- */

    for (user, _, _) in strategy {
        etx.user_print_balances(user)?;
    }

    // Inspect resulting balances
    for ((user, amts_in, _), (start, start_lp)) in zip(strategy, start_balances) {
        let (end, end_lp) = etx.user_balances(user)?;

        assert_eq!(start_lp, end_lp);

        for i in [0, 1] {
            if amts_in[i] > 0 {
                // We paid in with token indexed i
                assert_eq!(end[i], start[i].strict_sub(amts_in[i]), "Something wrong with {}", user.name);

                // The other token index:
                let other = 1 - i;

                // We should have gotten some amount of the other token
                assert!(end[other] > start[other]);
            }
        }

        // If we didn't put anything in, no swap should happen and we should
        // have what we started with.
        if amts_in[0] == 0 && amts_in[1] == 0 {
            assert_eq!(start, end);
        }
    }

    Ok(())
}

/////////////////////// COMPARE /////////////////////////////

// Do a pair of swaps both serially and concurrently, comparing the results.
#[test]
fn compare_swap() {
    println!("\n[Serial]");
    let serial = transact_assert_success(|| {
        let etx = setup_etx_primed()?;

        serial_swap(&etx)?;

        etx.record_stats(CAROL, DAVE)
    });

    println!("\n[Concurrent]");
    let concurrent = transact_assert_success(|| {
        let etx = setup_etx_primed()?;

        concurrent_swap(&etx)?;

        etx.record_stats(CAROL, DAVE)
    });

    println!("swap serial: \n{}", serial);
    println!("swap concurrent: \n{}", concurrent);

    assert_eq!(serial, concurrent);
}
