// A `#[slot(N)]` with N >= 2^32 must be a compile error. The slot index is a
// `u32` (the emit domain), so an out-of-range literal cannot silently truncate
// and alias onto another slot.
#[derive(sdk::Storage)]
pub struct Bad {
    #[slot(4294967296)] a: sdk::storage::Value<u64>,
}

fn main() {}
