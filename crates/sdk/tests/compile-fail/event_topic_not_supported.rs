// A `#[topic]` field whose type has no `IntoTopic` impl. Pins two things:
// the error lands on the field rather than the `derive(Event)` line, and it
// is a trait bound (E0277) rather than method resolution (E0599). The derive
// used to call `.into_topic()` as a method, so the failure was E0599 and
// `IntoTopic`'s `on_unimplemented` never fired at all.
#![allow(unexpected_cfgs)]

use sdk::*;

#[derive(Event, Clone, Debug)]
pub struct Ping {
    #[topic]
    pub tag: u8,
}

fn main() {}
