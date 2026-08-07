extern crate alloc;

use alloc::vec::Vec;
use sdk::*;

#[contract_type]
pub struct Payload {
    pub count: u32,
}

#[contract_type]
pub enum Choice {
    Left = 1,
    Right = 2,
}

#[contract_type]
pub enum Envelope {
    Empty = 0,
    Wrapped { payload: Payload } = 1,
    Full { payload: Payload, choice: Choice } = 2,
    Pair(Payload, Choice) = 3,
    Single(Payload) = 4,
}

pub type PayloadPair = (Payload, Option<Payload>);
pub type Byte = u8;
pub type Blob = Vec<Byte>;
pub type Hash = [Byte; 32];
pub type BlobEnvelope = (Blob, Hash);

/// Declared on the contract below via `emits(...)`; lands in the spec as a
/// regular (declared) event.
#[derive(Event)]
pub struct PayloadEvent<'a> {
    #[topic]
    pub origin: &'a ContractAddress,
    pub payload: Payload,
}

/// Not declared anywhere; `export` forces it into the spec of any contract
/// that links this code.
#[derive(Event)]
#[event_spec(export)]
pub struct AuditEvent {
    pub tag: u64,
}

/// Event declared only on `UnimplementedInterface` below.  Because that trait
/// is never implemented by `SpecNamedTypesContract`, spec shaking must remove
/// this event from the final canonicalized spec.
#[derive(Event)]
pub struct GhostEvent {
    pub code: u32,
}

/// A second `#[contract]` trait that carries `emits(GhostEvent)` but is
/// intentionally NOT implemented by `SpecNamedTypesContract`.  Its candidate
/// spec fragments (interface + method + GhostEvent def) co-travel into the
/// raw WASM section, but the absence of an `InterfaceAnchor` causes spec
/// shaking to drop both the interface and GhostEvent from the final spec.
/// This exercises the original bug: unimplemented-trait events must not leak.
#[contract(emits(GhostEvent))]
pub trait UnimplementedInterface {
    fn ghost_ping() -> u32;
}

#[derive(Default)]
pub struct SpecNamedTypesContract;

#[contract(version = "0.3.0", emits(PayloadEvent))]
impl SpecNamedTypesContract {
    pub fn echo(payload: Payload) -> Payload {
        payload
    }

    pub fn choose(choice: Choice) -> Choice {
        choice
    }

    pub fn wrap(flag: bool, payload: Payload) -> Envelope {
        if flag {
            Envelope::Wrapped { payload }
        } else {
            Envelope::Empty
        }
    }

    pub fn echo_pair(payloads: PayloadPair) -> PayloadPair {
        payloads
    }

    pub fn echo_blob(blob: Blob) -> Blob {
        blob
    }

    pub fn echo_hash(hash: Hash) -> Hash {
        hash
    }

    pub fn echo_blob_envelope(envelope: BlobEnvelope) -> BlobEnvelope {
        envelope
    }
}
