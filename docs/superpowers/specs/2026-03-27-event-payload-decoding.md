# Event Payload Decoding Design Spec

## Overview

Extend the `Event` trait and `#[derive(Event)]` macro to support typed payload decoding. The macro generates a payload struct and the trait provides `decode_payload`, enabling typed decoding of event payloads without requiring the contract author to think about it.

## Problem

Events have topic fields (hashed to 32 bytes, not reconstructible) and payload fields (XDR-serialized). The test infrastructure (`filter_events`, `assert_emitted`) needs typed decoding of payloads, but the current `Event` trait has no facility for this. The event struct itself can't be decoded from raw bytes because the topic fields are lossy.

## Design

### `Event` Trait Changes

The `Event` trait in `groundhog-event` gains an associated type and method:

```rust
pub trait Event: xdrs::WriteXDR {
    type Payload<'a>;

    fn num_topics(&self) -> u32;

    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        let _ = index;
        None
    }

    fn decode_payload<'a>(bytes: &'a [u8]) -> Result<Self::Payload<'a>, xdrs::ReadError>;
}
```

This is a breaking change to the `Event` trait. All existing manual `Event` impls must add `type Payload` and `decode_payload`. This is intentional — payload decoding is a core part of the event contract.

### Generated Payload Struct

For this input:

```rust
#[derive(Event)]
struct TransferEvent<'a> {
    #[topic]
    from: &'a ContractAddress,
    #[topic]
    to: &'a ContractAddress,
    amount: u128,
    memo: &'a [u8],
}
```

The macro generates (in addition to existing `WriteXDR` and `SIGNATURE`):

```rust
#[derive(ReadXDR)]
pub struct TransferEventPayload<'a> {
    pub amount: u128,
    pub memo: &'a [u8],
}

impl<'a> Event for TransferEvent<'a> {
    type Payload<'p> = TransferEventPayload<'p>;

    fn decode_payload<'p>(bytes: &'p [u8]) -> Result<TransferEventPayload<'p>, ReadError> {
        decode_xdr(bytes)
    }

    // ... num_topics, topic as before ...
}
```

### Naming Convention

The payload struct is named `{EventName}Payload`. It is public so consumers can name it if needed, but the typical usage path is through `<E as Event>::Payload<'_>`.

### Lifetime Handling

The payload struct copies the event's lifetime parameters, but they all unify to the decode buffer lifetime at the call site:

- **No lifetimes** (`struct SimpleEvent`): payload struct has no lifetime params. The GAT `Payload<'p>` just ignores `'p`.
- **One lifetime** (`struct E<'a>`): payload struct has `<'a>`, mapped to `'p` in the GAT.
- **Multiple lifetimes** (`struct E<'a, 'b>`): payload struct has `<'a, 'b>`, both mapped to `'p`: `type Payload<'p> = EPayload<'p, 'p>`. All lifetimes unify to the single decode buffer. If this causes a compile error in some edge case, that's acceptable.

### Payload Struct Field Types

The payload struct fields are **exact copies** of the non-`#[topic]` fields from the event struct — same types, same order. No borrow-to-owned conversion. The `ReadXDR` derive on the payload struct handles deserialization.

### Crate Placement

| Component | Crate | Rationale |
|---|---|---|
| `Event` trait (updated) | `groundhog-event` | Already lives here |
| Generated payload struct | user's crate | Emitted by macro alongside the event struct |
| Re-export of `Event` | `sdk::event` | Already re-exported |

### Test Infrastructure Integration

`FrameResult` in `native_exec` gets a new assertion method:

```rust
pub fn assert_event_payload<E>(&self, index: usize, check: impl FnOnce(&E::Payload<'_>))
where
    E: Event,
{
    let event = &self.events[index];
    let topics_len = 32 * event.num_topics as usize;
    let payload_bytes = &event.serialized_event[topics_len..];
    let decoded = E::decode_payload(payload_bytes).unwrap();
    check(&decoded);
}
```

Usage in tests:

```rust
result.assert_event_payload::<TransferEvent>(0, |p| {
    assert_eq!(p.amount, 42);
    assert_eq!(p.memo, b"hello");
});
```

The contract writer never names or thinks about `TransferEventPayload` — it's there for the infrastructure.

### Updating Existing Manual `Event` Impls

Existing impls (ERC20 `TransferEvent`, `ApprovalEvent`, test events) must add:

1. A payload struct with the non-topic fields
2. `type Payload<'a> = ...` and `fn decode_payload(...)` in the `Event` impl

For events that already have a hand-written `WriteXDR` (like the ERC20 events), the payload struct fields must match what `WriteXDR` serializes.

## What This Does NOT Do

- Does not reconstruct topic fields from raw bytes (topics are hashed, lossy).
- Does not replace existing `assert_event` or `assert_event_data` methods.
