# Typed event queries via `TypedEvent` — design

Date: 2026-07-06
Status: approved (pending spec review)
Source finding: `docs/sdk-review-2026-07-06.md` — [sdk-api-surface/silent-failure]

## Problem

`FrameResult::filter_events::<E>` and `assert_emitted::<E>`
(`crates/native-exec/src/frame_result.rs`, re-exported on `TransactResult`)
bound `E: xdrs::ReadXDROwned + Event` and decode the whole struct from the
event's payload bytes via a shared helper `decode_as`, swallowing decode
errors with `.ok()?` so foreign events are skipped.

`#[derive(Event)]` deliberately partitions `#[topic]` fields out of the
payload: the generated `WriteXDR` writes only non-topic fields, `ReadXDR` is
derived only on a separate `{Ident}Payload` struct, and the trait exposes
`Event::decode_payload` / `type Payload<'a>` for the split. `decode_as`
ignores that split, producing two verified failure modes:

1. **Doesn't compile.** No `#[derive(Event)]` type gets a `ReadXDR` impl, so
   no derived event satisfies `ReadXDROwned` (a blanket impl over
   `for<'r> ReadXDR<'r>`). Reference `#[topic]` fields (the
   groundhog-erc20 `TransferEvent` pattern) can never satisfy it.
2. **Silently fails.** Manually adding `#[derive(ReadXDR)]` to an event with
   owned `#[topic]` fields compiles, but decoding then reads topic fields
   from payload bytes where they were never written. The error is swallowed,
   so `filter_events` returns an empty `Vec` and `assert_emitted` panics
   with "got 0" for an event that was emitted correctly. Reproduced
   empirically: `assert_event::<Bid>` (which uses `Event::decode_payload`)
   decodes the same emitted event that `filter_events::<Bid>` cannot see.

A full `E` can never be reconstructed from a raw event: non-32-byte topic
values are SHA-256 hashes (`crates/sdk/src/event.rs`). The API must return
what exists on the wire — raw topics plus decoded payload — which is exactly
`TypedEvent<'a, E>`, already used correctly by `assert_event`.

No callers of `filter_events`/`assert_emitted` exist outside this repo's own
harness tests (checked sibling repos including groundhog-rpc), so the
signature change is safe.

## Design

### 1. `groundhog-event`: non-panicking `TypedEvent` construction (additive)

In `crates/event/src/emitted_event.rs`, refactor `TypedEvent::new`'s body
into a private `build(raw) -> Result<Self, BuildError>`, where `BuildError`
is a small internal enum covering the three failure cases:

- malformed event (claimed topics don't fit the body),
- topic-0 discriminant mismatch (when `E::signature()` is `Some`),
- payload decode error.

`BuildError` implements `Display` with text matching today's panic messages.

- `new(raw)` (unchanged signature, `#[track_caller]`): `build(raw)` +
  `unwrap_or_else(|e| panic!(...))` — diagnostics preserved.
- New `pub fn try_new(raw: &'a TxEmittedEvent) -> Option<Self>`:
  `build(raw).ok()` — documented as the non-panicking "is this event an
  `E`?" probe for scanning streams that contain foreign events.

### 2. `native-exec`: rebase both query APIs (breaking)

In `crates/native-exec/src/frame_result.rs`, delete `decode_as` and
`topic0_matches`. New signatures:

```rust
pub fn filter_events<E: Event>(
    &self,
    address: &ContractAddress,
) -> Vec<TypedEvent<'_, E>>

pub fn assert_emitted<E: Event>(
    &self,
    address: &ContractAddress,
    predicate: impl Fn(&TypedEvent<'_, E>) -> bool,
)
```

Both iterate `self.events`, filter by `contract_address == address`, and
keep events where `TypedEvent::try_new::<E>` returns `Some`.
`assert_emitted` keeps its current "expected exactly 1 matching event, got
{n}" panic and returns `()` (returning the match was considered and
deliberately dropped from scope). The delegating wrappers on
`TransactResult` (`crates/native-exec/src/transact_result.rs`) update
identically. No `ReadXDROwned` bound remains, removing the silent-failure
workaround at the type level.

### 3. Semantics note (documented on both methods)

For events whose `Event::signature()` is `None`, matching is
payload-decode-only. The old instance-based `num_topics` cross-check is
unrepresentable without a decoded `E` (`num_topics` takes `&self` — this is
the mechanism that caused the bug). This is the same rule
`TypedEvent::new`/`assert_event` already follow, so the two typed query
families now agree.

### 4. Tests

- **`crates/native-exec/tests/fluent_api.rs`**: update call sites —
  predicates and result inspection go through `TypedEvent`
  (`t.payload() == &EventA(42)`-style). The existing malformed-event and
  foreign-event skip tests keep their intent, now exercised through
  `try_new`.
- **`crates/sdk/tests/events.rs`** (where the derive and `TestEnvironment`
  are both available) — regression tests the old suite lacked:
  - a `#[derive(Event)]` type with an **owned** `#[topic]` field:
    `filter_events` finds it; `assert_emitted` matches on payload fields and
    on topic bytes;
  - a `#[derive(Event)]` type with **reference** `#[topic]` fields (the
    erc20 pattern): both APIs compile and find the emitted event;
  - a different derived event emitted at the same address is skipped by
    `filter_events::<E>` and not confused for `E` (distinct signatures).

## Out of scope

- Making `assert_emitted` return the matched `TypedEvent` (no owned
  conversion exists on `TypedEvent`, and a borrowed return was judged
  unnecessary for now).
- A static `NUM_TOPICS` on the `Event` trait to tighten signature-less
  matching.
- Migrating example-contract tests (e.g. swap's raw `exec_tx2` event
  closures) onto the fixed APIs.
