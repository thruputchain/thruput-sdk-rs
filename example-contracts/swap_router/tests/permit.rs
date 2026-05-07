mod common;
use common::*;

#[rustfmt::skip]
#[test]
fn purchase_with_permit() {
    transact_assert_success(|| {
        let etx = setup_etx()?;
        _purchase_with_permit(&etx)
    });
}

fn _purchase_with_permit(etx: &Etx) -> Result<()> {
    let ctx = &etx.ctx;

    let router = ctx.router();

    etx.exec_user_tx(PRIMER, |_user| {

        let me = get_current_address();

        let amounts_desired = [ctx.token0().get_balance(), ctx.token1().get_balance()];
        let amounts_min = [0, 0];

        // Transfer our AB-tokens to the pair contract
        ctx.token0()
            .transfer(&ctx.pair_addr, amounts_desired[0].try_into().unwrap());
        ctx.token1()
            .transfer(&ctx.pair_addr, amounts_desired[1].try_into().unwrap());

        // Where the LP tokens go
        let recipient = &me;

        let permits = [
            Permit::new(&me, &ctx.token_addrs[0], amounts_desired[0]),
            Permit::new(&me, &ctx.token_addrs[1], amounts_desired[1]),
        ];

        router.add_liquidity(
            &ctx.token_addrs[0],
            &ctx.token_addrs[1],
            amounts_desired,
            amounts_min,
            recipient,
            permits,
        );
    })?;

    /* ----- */ etx.advance_block(); /* ----- */

    Ok(())
}
