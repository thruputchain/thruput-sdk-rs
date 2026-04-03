use sdk::*;

#[derive(Default)]
pub struct LegacyPathContract;

#[contract(storage = false)]
impl LegacyPathContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {
    let _ = exports::PING_EXPORT_NUM;
}
