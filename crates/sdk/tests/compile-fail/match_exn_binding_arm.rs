use sdk::*;

#[contract_exception]
pub enum E {
    A = 1,
}

fn main() {
    let e = CallError::Invalid;
    // A bare identifier is a binding, not a path, so there is nothing to
    // infer the exception type from. `_` is the untyped escape hatch.
    let _n: u32 = match_exn!(e, {
        caught => 1,
        _ => 0,
    });
}
