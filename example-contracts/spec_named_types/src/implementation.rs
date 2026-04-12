extern crate alloc;

use alloc::vec::Vec;
use sdk::*;

#[contracttype]
pub struct Payload {
    pub count: u32,
}

#[contracttype]
pub enum Choice {
    Left = 1,
    Right = 2,
}

#[contracttype]
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

#[derive(Default)]
pub struct SpecNamedTypesContract;

#[contract(version = "0.3.0")]
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
