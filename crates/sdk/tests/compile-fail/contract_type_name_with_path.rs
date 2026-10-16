use thruput_sdk::*;

// `name` replaces only the final segment of the spec id; the module path comes
// from the declaration, so a name may not spell path segments of its own.
#[contract_type(name = "spelled::Out")]
pub struct SpelledOut {
    pub v: u32,
}

fn main() {}
