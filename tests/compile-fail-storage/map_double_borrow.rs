use sdk::storage::*;

fn main() {
    let mut m: Map<u64, Value<u64>> = Map::new(&Keypath::new());
    let mut slot = m.at_mut(&10u64);
    slot.set(&10u64);

    let mut slot2 = m.at_mut(&12u64);
    slot2.set(&15u64);

    slot.set(&15u64);
}
