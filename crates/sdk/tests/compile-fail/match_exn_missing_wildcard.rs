use sdk::*;

#[contract_exception]
pub enum E {
    A = 1,
}

fn main() {
    let e = CallError::Invalid;
    // Arms are tried in order rather than checked for exhaustiveness, so a
    // `_` arm is required.
    let _n: u32 = match_exn!(e, {
        E::A => 1,
    });
}
