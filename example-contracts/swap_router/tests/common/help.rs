// Helpers for testing the swap contract

use super::*;

pub fn is_pair_error_and(
    e: ISwapV2PairClientError,
    f: impl Fn(ISwapV2PairError) -> bool,
) -> bool {
    match e {
        ISwapV2PairClientError::ISwapV2PairError(e) => f(e),
        _ => false,
    }
}

pub fn print_balances(name: &str, bals: (Amounts, Amount), symbols: [Symbol; 2]) {
    println!(
        "{name:>10}: {:>10.1} {}  {:>10.1} {}  {:>10.1} LP",
        decimalized(bals.0[0]),
        symbols[0],
        decimalized(bals.0[1]),
        symbols[1],
        decimalized(bals.1),
    );
}

pub fn balances_sum(a: Amounts, b: Amounts) -> Amounts {
    [a[0].strict_add(b[0]), a[1].strict_add(b[1])]
}

pub fn balances_diff(a: Amounts, b: Amounts) -> [i128; 2] {
    [0, 1].map(|i| balance_diff(a[i], b[i]))
}

fn balance_diff(a: Amount, b: Amount) -> i128 {
    let a: i128 = a.try_into().unwrap();
    let b: i128 = b.try_into().unwrap();
    a.strict_sub(b)
}

pub fn balances_strict_sub(a: Amounts, b: Amounts) -> Amounts {
    [0, 1].map(|i| a[i].strict_sub(b[i]))
}

pub fn amounts_decimalized(amts: Amounts) -> (f64, f64) {
    (decimalized(amts[0]), decimalized(amts[1]))
}

pub fn show_result(r: &TransactResult) {
    let frame = r.frame();

    if frame.is_success() {
        println!("SUCCESS");
    } else if frame.is_fault() {
        println!("FAULT");
    } else if frame.is_abort() {
        print!("EXCEPTION: ");
        show_exn(r);
    } else {
        unreachable!();
    }
}

pub fn show_exn(r: &TransactResult) {
    let exn = RawException::from_abort_buf(r.frame().abort_buffer()).unwrap();

    if let Some(exn) = exn.try_as::<ERC20Error>() {
        println!("ERC20: {:?}", exn);
    } else if let Some(exn) = exn.try_as::<ISwapV2PairError>() {
        println!("ISwapV2Pair: {:?}", exn);
    } else if let Some(exn) = exn.try_as::<LibraryExn>() {
        println!("SDK error: {}", exn);
    } else {
        println!("<unknown error>");
    }
}
