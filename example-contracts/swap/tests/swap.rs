mod common;
use common::*;

fn calculate_swap(amounts_in: Amounts) -> Amounts {
    [(amounts_in[1] / 10) * 9, (amounts_in[0] / 10) * 9]
}

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
        serial_swap_user(&etx, user, amounts_in)?;

        Ok(())
    });
}

fn serial_swap(etx: &Etx) -> Result<()> {
    etx.user_print_balances(CAROL)?;
    etx.user_print_balances(DAVE)?;

    // Carol does a swap
    let user = CAROL;
    let amounts_in = [2 * UNIT_AMT, 0];
    serial_swap_user(etx, user, amounts_in)?;

    // Dave swaps in the other direction
    let user = DAVE;
    let amounts_in = [0, 2 * UNIT_AMT];
    serial_swap_user(etx, user, amounts_in)?;

    Ok(())
}

#[rustfmt::skip]
fn serial_swap_user(
    etx: &Etx,
    user: User,
    amounts_in: Amounts,
) -> Result<()> {
    let ctx = &etx.ctx;

    // Send the purchasing funds to the swap-pair contract
    etx.exec_user_tx(user, |_| {
        ctx.token0().transfer(&ctx.pair_addr, amounts_in[0])
    })?;
    etx.exec_user_tx(user, |_| {
        ctx.token1().transfer(&ctx.pair_addr, amounts_in[1])
    })?;

    /* ----- */ etx.advance_block(); /* ----- */

    let amounts_out = calculate_swap(amounts_in);

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
