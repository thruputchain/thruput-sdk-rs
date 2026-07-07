# Typed Event Queries via TypedEvent — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `FrameResult::filter_events`/`assert_emitted` (and their `TransactResult` wrappers) work with `#[derive(Event)]` + `#[topic]` events by rebasing them on `TypedEvent` instead of whole-struct `ReadXDROwned` decoding.

**Architecture:** Add a non-panicking `TypedEvent::try_new` to `groundhog-event` (shared `build` helper keeps `new`'s panic diagnostics). Rewrite the two query APIs in `groundhog-native-exec` to filter via `try_new`, returning `Vec<TypedEvent<'_, E>>` / taking `Fn(&TypedEvent<'_, E>) -> bool` predicates, with no `ReadXDROwned` bound. Add regression tests with derived events (owned and reference `#[topic]` fields) in the sdk test suite.

**Tech Stack:** Rust workspace; crates `groundhog-event` (no_std), `groundhog-native-exec`, `sdk`. Spec: `docs/superpowers/specs/2026-07-06-typed-event-queries-design.md`.

## Global Constraints

- Branch: `geoff/typed-event-queries` (already created; all commits go here).
- `crates/event` is `#![no_std]`: no `std`/`alloc` in its code or unit tests (its tests use `&'static` bodies; do not call `xdrs::cxs::encode` there).
- Panic message texts of `TypedEvent::new` must not change — downstream `#[should_panic(expected = ...)]` tests match substrings of them.
- `assert_emitted` keeps its exact panic text: `"expected exactly 1 matching event, got {n}"` and returns `()`.
- No new dependencies anywhere.
- Known pre-existing failure: the erc20_low_level full-suite test can flake when running the whole workspace suite; it is unrelated to this change.

---

### Task 1: `TypedEvent::try_new` in groundhog-event

**Files:**
- Modify: `crates/event/src/emitted_event.rs` (struct impl at lines 86-145, tests mod at lines 147-200)

**Interfaces:**
- Consumes: existing `EmittedEvent`, `Event`, `Topic` from this crate; `TxEmittedEvent` from `groundhog_xdr::event`.
- Produces: `pub fn TypedEvent::<E>::try_new(raw: &'a TxEmittedEvent) -> Option<TypedEvent<'a, E>>` — Task 2 calls this. `TypedEvent::new` keeps its exact signature and panic messages.

- [ ] **Step 1: Write the failing tests**

Append inside the existing `mod tests` in `crates/event/src/emitted_event.rs` (after `payload_is_some_for_well_formed_event`):

```rust
    const SIG: Topic = [0xAB; 32];

    /// Minimal Event impl with a static signature and a 4-byte big-endian
    /// u32 payload. Decode is hand-rolled so these no_std tests don't need
    /// cxs encode (which requires alloc).
    struct SigEvent;

    impl xdrs::WriteXDR for SigEvent {
        fn write_xdr<F, W: xdrs::Writer<F>>(
            &self,
            _w: &mut W,
        ) -> xdrs::WriteResult<()> {
            Ok(())
        }
    }

    impl Event for SigEvent {
        type Payload<'a> = u32;

        fn num_topics(&self) -> u32 {
            1
        }

        fn signature() -> Option<&'static Topic> {
            Some(&SIG)
        }

        fn decode_payload(bytes: &[u8]) -> Result<u32, xdrs::ReadError> {
            match <[u8; 4]>::try_from(bytes) {
                Ok(b) => Ok(u32::from_be_bytes(b)),
                Err(_) => Err(xdrs::ReadError::SourceEmpty {
                    requested: 4,
                    remaining: bytes.len(),
                }),
            }
        }
    }

    #[test]
    fn try_new_decodes_matching_event() {
        // Topic 0 = SIG (32 × 0xAB), payload = 7u32 big-endian.
        static BODY: [u8; 36] = {
            let mut b = [0xABu8; 36];
            b[32] = 0;
            b[33] = 0;
            b[34] = 0;
            b[35] = 7;
            b
        };
        let raw = raw_event(1, &BODY);
        let typed =
            TypedEvent::<SigEvent>::try_new(&raw).expect("should decode");
        assert_eq!(*typed.payload(), 7);
        assert_eq!(typed.topic(0), Some(&SIG));
    }

    #[test]
    fn try_new_rejects_signature_mismatch() {
        // Topic 0 is 32 × 0xCD ≠ SIG; payload bytes are valid u32.
        static BODY: [u8; 36] = [0xCD; 36];
        let raw = raw_event(1, &BODY);
        assert!(TypedEvent::<SigEvent>::try_new(&raw).is_none());
    }

    #[test]
    fn try_new_rejects_malformed_event() {
        // Claims 3 topics (96 bytes) but the body is only 40.
        static BODY: [u8; 40] = [0u8; 40];
        let raw = raw_event(3, &BODY);
        assert!(TypedEvent::<SigEvent>::try_new(&raw).is_none());
    }

    #[test]
    fn try_new_rejects_undecodable_payload() {
        // Topic 0 = SIG but only 3 payload bytes — decode_payload fails.
        static BODY: [u8; 35] = [0xAB; 35];
        let raw = raw_event(1, &BODY);
        assert!(TypedEvent::<SigEvent>::try_new(&raw).is_none());
    }

    #[test]
    #[should_panic(expected = "event type mismatch")]
    fn new_still_panics_on_signature_mismatch() {
        static BODY: [u8; 36] = [0xCD; 36];
        let raw = raw_event(1, &BODY);
        let _ = TypedEvent::<SigEvent>::new(&raw);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p groundhog-event`
Expected: compile error — `no function or associated item named 'try_new' found for struct 'TypedEvent'`.

- [ ] **Step 3: Implement `build`/`try_new`**

In `crates/event/src/emitted_event.rs`, insert immediately above `pub struct TypedEvent` (currently line 79):

```rust
/// Why constructing a `TypedEvent` from a raw event failed. Internal:
/// surfaced only through [`TypedEvent::new`]'s panic message (the `Display`
/// texts below are that message and must not change) or discarded by
/// [`TypedEvent::try_new`].
enum BuildError {
    /// The event's claimed topics don't fit its body.
    Malformed { num_topics: u32, body_len: usize },
    /// Topic 0 doesn't match `E`'s static discriminant.
    SignatureMismatch {
        actual: Option<Topic>,
        expected: &'static Topic,
        type_name: &'static str,
    },
    /// The payload bytes don't decode as `E::Payload`.
    Decode(xdrs::ReadError),
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuildError::Malformed {
                num_topics,
                body_len,
            } => write!(
                f,
                "TypedEvent::new: malformed event — {} topics do not fit a \
                 {}-byte body",
                num_topics, body_len,
            ),
            BuildError::SignatureMismatch {
                actual,
                expected,
                type_name,
            } => write!(
                f,
                "TypedEvent::new: event type mismatch — topic 0 {:?} does \
                 not match the expected discriminant {:?} for {}",
                actual, expected, type_name,
            ),
            BuildError::Decode(e) => {
                write!(f, "failed to decode event payload: {}", e)
            }
        }
    }
}
```

Then replace the body of `impl<'a, E: Event> TypedEvent<'a, E>`'s `new` (currently lines 87-124, keeping `payload`, `topic`, `num_topics`, `raw` unchanged) with:

```rust
    /// Decode a raw event into a typed view, or say why it isn't an `E`.
    /// Shared engine behind [`new`](Self::new) (panics with the error's
    /// `Display` text) and [`try_new`](Self::try_new) (discards it).
    fn build(raw: &'a TxEmittedEvent) -> Result<Self, BuildError> {
        let emitted = EmittedEvent::from(raw);
        // Reject a malformed event up front, before the signature/payload
        // checks (which would otherwise report a misleading "type mismatch"
        // or decode a silently-empty payload).
        let payload_bytes =
            emitted.payload().ok_or(BuildError::Malformed {
                num_topics: raw.num_topics,
                body_len: raw.serialized_event.len(),
            })?;
        if let Some(expected) = E::signature() {
            match emitted.topic(0) {
                Some(actual) if actual == expected => {}
                other => {
                    return Err(BuildError::SignatureMismatch {
                        actual: other.copied(),
                        expected,
                        type_name: core::any::type_name::<E>(),
                    });
                }
            }
        }
        let payload =
            E::decode_payload(payload_bytes).map_err(BuildError::Decode)?;
        Ok(Self { emitted, payload })
    }

    /// Decode a raw event into a typed view.
    ///
    /// When `E` declares a static topic-0 discriminant (via
    /// [`Event::signature`]), the raw event's topic 0 is verified against it
    /// first, so a payload that merely *shares a shape* with `E` cannot be
    /// silently mistyped as `E`. Panics on a discriminant mismatch or on
    /// payload decode failure.
    #[track_caller]
    pub fn new(raw: &'a TxEmittedEvent) -> Self {
        Self::build(raw).unwrap_or_else(|e| panic!("{}", e))
    }

    /// Non-panicking twin of [`new`](Self::new): `Some` only if the event is
    /// well-formed, its topic 0 matches `E`'s declared discriminant (when
    /// one exists), and its payload decodes as `E::Payload`. The probe for
    /// scanning event streams that contain foreign events.
    ///
    /// When [`Event::signature`] is `None`, matching is payload-decode-only:
    /// with no decoded `E` instance there is nothing to cross-check topic
    /// counts against.
    pub fn try_new(raw: &'a TxEmittedEvent) -> Option<Self> {
        Self::build(raw).ok()
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p groundhog-event`
Expected: PASS, including the 3 pre-existing `EmittedEvent` tests and the 5 new ones.

- [ ] **Step 5: Check no behavior change for existing `new` users**

Run: `cargo test -p sdk --test events`
Expected: PASS (this suite exercises `TypedEvent`-based `assert_event`, including its `#[should_panic]` mismatch tests, against the unchanged panic texts).

- [ ] **Step 6: Commit**

```bash
git add crates/event/src/emitted_event.rs
git commit -m "event: add non-panicking TypedEvent::try_new"
```

---

### Task 2: Rebase filter_events/assert_emitted on TypedEvent

**Files:**
- Modify: `crates/native-exec/src/frame_result.rs` (delete lines 7-54: `topic0_matches` + `decode_as`; rewrite `filter_events` at 258-269 and `assert_emitted` at 310-333)
- Modify: `crates/native-exec/src/transact_result.rs` (wrappers at 142-147 and 169-178)
- Test: `crates/native-exec/tests/fluent_api.rs` (call sites listed below)

**Interfaces:**
- Consumes: `TypedEvent::try_new` from Task 1 (`fn try_new(raw: &'a TxEmittedEvent) -> Option<TypedEvent<'a, E>>`).
- Produces (Task 3 relies on these exact signatures, on both `FrameResult` and `TransactResult`):
  - `pub fn filter_events<E: Event>(&self, address: &ContractAddress) -> Vec<TypedEvent<'_, E>>`
  - `pub fn assert_emitted<E: Event>(&self, address: &ContractAddress, predicate: impl Fn(&TypedEvent<'_, E>) -> bool)`

- [ ] **Step 1: Update the fluent_api tests to the new API (the failing "red")**

In `crates/native-exec/tests/fluent_api.rs`, make these exact replacements (predicates receive `&TypedEvent<'_, E>`; for the hand-written test events `Payload<'a> = Self`, so `t.payload()` is `&TestValue`/`&EventA`/`&EventB`):

Line 710:
```rust
    result.assert_emitted::<TestValue>(&TEST_ADDR, |t| t.payload().0 == 20);
```

Lines 713-715:
```rust
    result
        .frame()
        .assert_emitted::<TestValue>(&TEST_ADDR, |t| t.payload().0 == 30);
```

Line 732:
```rust
    result.assert_emitted::<TestValue>(&TEST_ADDR, |t| t.payload().0 == 999);
```

Line 743: unchanged (`|_| true`).

Lines 862-863 (in `test_filter_events`):
```rust
    let values = result.filter_events::<TestValue>(&TEST_ADDR);
    let payloads: Vec<&TestValue> =
        values.iter().map(|t| t.payload()).collect();
    assert_eq!(payloads, vec![&TestValue(10), &TestValue(20), &TestValue(30)]);
```

Lines 999-1005 (in `test_filter_events_by_topic`):
```rust
    // filter_events<EventA> should only return the two EventA instances
    let a_events = result.filter_events::<EventA>(&TEST_ADDR);
    let a_payloads: Vec<&EventA> =
        a_events.iter().map(|t| t.payload()).collect();
    assert_eq!(a_payloads, vec![&EventA(10), &EventA(30)]);

    // filter_events<EventB> should only return the one EventB instance
    let b_events = result.filter_events::<EventB>(&TEST_ADDR);
    let b_payloads: Vec<&EventB> =
        b_events.iter().map(|t| t.payload()).collect();
    assert_eq!(b_payloads, vec![&EventB(20)]);
```

Lines 1023-1029 (in `test_filter_events_topic_mismatch_skips`):
```rust
    // EventA declares a signature, but the raw event has no topics → no match
    let events = result.filter_events::<EventA>(&TEST_ADDR);
    assert!(events.is_empty());

    // TestValue has no signature: payload decode alone decides → match
    let events = result.filter_events::<TestValue>(&TEST_ADDR);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload(), &TestValue(42));
```

Lines 1063-1066:
```rust
    // assert_emitted for EventA should find EventA(10) but not EventB(20)
    result.assert_emitted::<EventA>(&TEST_ADDR, |t| t.payload().0 == 10);

    // assert_emitted for EventB should find EventB(20) but not EventA(10)
    result.assert_emitted::<EventB>(&TEST_ADDR, |t| t.payload().0 == 20);
```

Line 1091: unchanged (`|_| true`).

Line 1124:
```rust
    result.assert_emitted::<TestValue>(&TEST_ADDR, |t| t.payload().0 == 20);
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p groundhog-native-exec --test fluent_api 2>&1 | head -30`
Expected: compile errors — the old `filter_events` returns `Vec<E>` and old predicates take `&E`, so `t.payload()` / `Vec<&TestValue>` collections don't type-check.

- [ ] **Step 3: Rewrite the two APIs in frame_result.rs**

Delete `topic0_matches` and `decode_as` entirely (lines 7-54, including both doc comments). Replace `filter_events` (258-269) with:

```rust
    /// Collect events from `address` that decode as `E` via
    /// [`TypedEvent::try_new`]: well-formed, topic 0 matching `E`'s declared
    /// discriminant (when [`Event::signature`] returns one), payload decoding
    /// as `E::Payload`. Foreign and malformed events are skipped, never
    /// panicked on.
    ///
    /// For events whose `Event::signature()` is `None`, matching is
    /// payload-decode-only — there is no decoded `E` instance to cross-check
    /// topic counts against.
    pub fn filter_events<E>(
        &self,
        address: &ContractAddress,
    ) -> Vec<TypedEvent<'_, E>>
    where
        E: Event,
    {
        self.events
            .iter()
            .filter(|e| &e.contract_address == address)
            .filter_map(|e| TypedEvent::try_new(e))
            .collect()
    }
```

Replace `assert_emitted` (310-333) with:

```rust
    /// Assert that exactly one event from `address` decodes as `E` (per
    /// [`TypedEvent::try_new`] — see [`filter_events`](Self::filter_events)
    /// for the matching rules) and satisfies `predicate`. Panics if zero or
    /// more than one event matches.
    #[track_caller]
    pub fn assert_emitted<E>(
        &self,
        address: &ContractAddress,
        predicate: impl Fn(&TypedEvent<'_, E>) -> bool,
    ) where
        E: Event,
    {
        let match_count = self
            .events
            .iter()
            .filter(|e| &e.contract_address == address)
            .filter_map(|e| TypedEvent::try_new(e))
            .filter(|typed| predicate(typed))
            .count();
        assert!(
            match_count == 1,
            "expected exactly 1 matching event, got {}",
            match_count
        );
    }
```

(`use groundhog_event::{EmittedEvent, Event, TypedEvent};` at line 1 already covers the imports; the `xdrs::ReadXDROwned` bounds on `assert_return_with`/`assert_abort_with`/`assert_return_as`/`assert_abort_as` stay.)

- [ ] **Step 4: Update the TransactResult wrappers**

In `crates/native-exec/src/transact_result.rs`, replace the wrapper at 142-147 with:

```rust
    pub fn filter_events<E>(
        &self,
        address: &ContractAddress,
    ) -> Vec<TypedEvent<'_, E>>
    where
        E: Event,
    {
        self.frame.filter_events(address)
    }
```

and the wrapper at 169-178 with:

```rust
    #[track_caller]
    pub fn assert_emitted<E>(
        &self,
        address: &ContractAddress,
        predicate: impl Fn(&TypedEvent<'_, E>) -> bool,
    ) where
        E: Event,
    {
        self.frame.assert_emitted(address, predicate);
    }
```

If `xdrs` was imported only for these bounds, remove the now-unused import (compiler warning will say).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p groundhog-native-exec --test fluent_api`
Expected: PASS — all tests including the four `#[should_panic]` assert_emitted tests and `test_assert_emitted_skips_unrelated_malformed_event`.

Run: `cargo test -p groundhog-native-exec`
Expected: PASS (no other suite in the crate touches these APIs).

- [ ] **Step 6: Commit**

```bash
git add crates/native-exec/src/frame_result.rs crates/native-exec/src/transact_result.rs crates/native-exec/tests/fluent_api.rs
git commit -m "native-exec: rebase filter_events/assert_emitted on TypedEvent

The old ReadXDROwned-based decode read whole event structs from payload
bytes, which is incompatible with #[derive(Event)]'s topic/payload split:
derived events either failed to compile or silently never matched. Matching
now goes through TypedEvent::try_new (signature check + decode_payload),
the same path assert_event already used."
```

---

### Task 3: Regression tests with derived events

**Files:**
- Modify: `crates/sdk/tests/events.rs` (append at end of file; `TEST_ADDR` is defined at line 469, `#[derive(Event)]`, `log_event`, `TestEnvironment` all already in scope via the file's existing imports)

**Interfaces:**
- Consumes: Task 2's `filter_events`/`assert_emitted` signatures on `TransactResult` (via `sdk`'s mock re-export), `#[derive(Event)]`'s generated `{Ident}Payload` structs (fields accessible in the same module), `Event::SIGNATURE` const.
- Produces: nothing downstream — final regression coverage for the spec's finding.

- [ ] **Step 1: Write the tests (failing only if Tasks 1-2 regressed)**

Append at the end of `crates/sdk/tests/events.rs`:

```rust
//-------------------------------------------------------------------------
// Regression: filter_events/assert_emitted work with #[derive(Event)]
// events, whose #[topic] fields are excluded from the payload bytes.
// Previously these APIs required whole-struct ReadXDR and either failed to
// compile (reference topics) or silently matched nothing (owned topics).
// See docs/superpowers/specs/2026-07-06-typed-event-queries-design.md.

#[derive(Event)]
struct OwnedTopicEvent {
    #[topic]
    bidder: ContractAddress,
    amount: u128,
}

// Same payload shape as OwnedTopicEvent (a lone u128) but a distinct type,
// so a distinct topic-0 signature.
#[derive(Event)]
struct OtherAmountEvent {
    amount: u128,
}

#[derive(Event)]
struct RefTopicEvent<'a> {
    #[topic]
    from: &'a ContractAddress,
    #[topic]
    to: &'a ContractAddress,
    amount: u128,
    memo: &'a [u8],
}

#[test]
fn filter_events_finds_derived_event_with_owned_topic() {
    let env = TestEnvironment::no_cost();
    let bidder = ContractAddress::new([7u8; 32]);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            log_event(&OwnedTopicEvent { bidder, amount: 5 });
        })
    });
    result.assert_success();

    let events = result.filter_events::<OwnedTopicEvent>(&TEST_ADDR);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload().amount, 5);
    assert_eq!(events[0].topic(0).unwrap(), OwnedTopicEvent::SIGNATURE);
    assert_eq!(events[0].topic(1).unwrap(), &*bidder);

    result.assert_emitted::<OwnedTopicEvent>(&TEST_ADDR, |t| {
        t.payload().amount == 5 && t.topic(1).unwrap() == &*bidder
    });
}

#[test]
fn filter_events_finds_derived_event_with_ref_topics() {
    let env = TestEnvironment::no_cost();
    let from = ContractAddress::new([1u8; 32]);
    let to = ContractAddress::new([2u8; 32]);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            log_event(&RefTopicEvent {
                from: &from,
                to: &to,
                amount: 42,
                memo: b"hi",
            });
        })
    });
    result.assert_success();

    let events = result.filter_events::<RefTopicEvent<'_>>(&TEST_ADDR);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload().amount, 42);
    assert_eq!(events[0].payload().memo, b"hi");
    assert_eq!(events[0].topic(1).unwrap(), &*from);
    assert_eq!(events[0].topic(2).unwrap(), &*to);

    result
        .assert_emitted::<RefTopicEvent<'_>>(&TEST_ADDR, |t| {
            t.payload().amount == 42
        });
}

#[test]
fn filter_events_skips_foreign_derived_events() {
    let env = TestEnvironment::no_cost();
    let bidder = ContractAddress::new([7u8; 32]);
    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            log_event(&OwnedTopicEvent { bidder, amount: 5 });
            log_event(&OtherAmountEvent { amount: 5 });
        })
    });
    result.assert_success();
    result.assert_event_count(2);

    // Payload-compatible but differently-signed events are not confused.
    let bids = result.filter_events::<OwnedTopicEvent>(&TEST_ADDR);
    assert_eq!(bids.len(), 1);
    let others = result.filter_events::<OtherAmountEvent>(&TEST_ADDR);
    assert_eq!(others.len(), 1);
    assert_eq!(others[0].payload().amount, 5);

    // Exactly one OwnedTopicEvent despite the payload-compatible sibling.
    result.assert_emitted::<OwnedTopicEvent>(&TEST_ADDR, |_| true);
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p sdk --test events`
Expected: PASS — the three new tests plus every pre-existing test in the suite.

If `ContractAddress` turns out not to be `Copy` at `log_event(&OwnedTopicEvent { bidder, ... })` inside the closure, bind it inside the closure instead: `let bidder = ContractAddress::new([7u8; 32]);` on the line above `log_event` (and compare against a fresh `ContractAddress::new([7u8; 32])` in the assertions).

- [ ] **Step 3: Commit**

```bash
git add crates/sdk/tests/events.rs
git commit -m "sdk: regression tests for typed event queries with #[derive(Event)] topics"
```

---

### Task 4: Workspace verification

**Files:** none (verification only; commit only if fmt/clippy require mechanical fixes)

**Interfaces:** n/a

- [ ] **Step 1: Format and lint**

Run: `cargo fmt --all -- --check`
Expected: no diff. If there is one, run `cargo fmt --all` and fold the fixes into an amend of the relevant task commit (or a `style:` commit if ambiguous).

Run: `cargo clippy -p groundhog-event -p groundhog-native-exec -p sdk --all-targets 2>&1 | tail -20`
Expected: no new warnings in the touched files.

- [ ] **Step 2: Full test pass over the affected crates**

Run: `cargo test -p groundhog-event -p groundhog-native-exec -p sdk`
Expected: PASS. (The erc20_low_level flake lives in a different crate's suite; if running the whole workspace instead, that one failure is known and pre-existing.)

- [ ] **Step 3: Confirm no stale API references remain**

Run: `grep -rn "decode_as\|topic0_matches" crates/native-exec/src/`
Expected: no output.

Run: `grep -rn "ReadXDROwned" crates/native-exec/src/frame_result.rs crates/native-exec/src/transact_result.rs`
Expected: hits only in `assert_return_with`/`assert_abort_with`/`assert_return_as`/`assert_abort_as` (frame_result.rs) and their wrappers — none on event APIs.
