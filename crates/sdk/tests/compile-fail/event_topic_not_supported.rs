// A `#[topic]` field whose type has no `IntoTopic` impl. The error must land
// on the field, not on the `derive(Event)` line.
#![allow(unexpected_cfgs)]

use sdk::*;

#[derive(Event, Clone, Debug)]
pub struct Ping {
    #[topic]
    pub tag: u8,
}

fn main() {}
