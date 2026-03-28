# Event Payload Decoding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Payload` GAT and `decode_payload` to the `Event` trait so event payloads can be decoded with borrowed types, and update the derive macro to generate a payload struct automatically.

**Architecture:** The `Event` trait gains `type Payload<'a>` and `fn decode_payload`. The `#[derive(Event)]` macro generates a `{Name}Payload` struct with the non-topic fields (same types, copied verbatim) and derives `ReadXDR` on it. All existing manual `Event` impls must be updated. The test infra gets `assert_event_payload` methods.

**Tech Stack:** `syn`, `quote`, `proc-macro2` (existing), `xdrs` (existing)

**Spec:** `docs/superpowers/specs/2026-03-27-event-payload-decoding.md`

---

### Task 1: Update `Event` trait with `Payload` GAT and `decode_payload`

**Files:**
- Modify: `groundhog-event/src/lib.rs`

- [ ] **Step 1: Add `Payload` and `decode_payload` to the `Event` trait**

Replace the `Event` trait in `groundhog-event/src/lib.rs` with:

```rust
/// Trait implemented by event types that can be emitted by contracts.
///
/// Provides the number of topics and their values. Topic 0 is
/// conventionally the event-type discriminant (e.g. a hash of the
/// event name). The XDR-serialized payload is everything *after*
/// the topics in the on-chain representation.
pub trait Event: xdrs::WriteXDR {
    /// The decoded payload type. Uses a GAT so it can borrow from
    /// the input bytes (e.g. `&'a [u8]` fields stay borrowed).
    type Payload<'a>;

    /// Number of topics this event carries (0..=3).
    fn num_topics(&self) -> u32;

    /// Retrieve the topic at the given index.
    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        let _ = index;
        None
    }

    /// Decode the payload portion of a serialized event.
    fn decode_payload<'a>(bytes: &'a [u8]) -> Result<Self::Payload<'a>, xdrs::ReadError>;
}
```

- [ ] **Step 2: Verify it compiles (expect downstream failures)**

Run: `cargo check -p groundhog-event`
Expected: compiles. Downstream crates will fail until their impls are updated.

- [ ] **Step 3: Commit**

```bash
git add groundhog-event/src/lib.rs
git commit -m "feat: add Payload GAT and decode_payload to Event trait"
```

---

### Task 2: Update `#[derive(Event)]` macro to generate payload struct

**Files:**
- Modify: `macros/src/derive_event.rs`

- [ ] **Step 1: Update `expand` to generate the payload struct and `EventPayload` impl**

Replace the contents of `macros/src/derive_event.rs` with:

```rust
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Fields, GenericParam, Result};

/// Returns (topic_fields, payload_fields).
fn partition_fields(
    fields: &Fields,
) -> Result<(Vec<&syn::Field>, Vec<&syn::Field>)> {
    let Fields::Named(named) = fields else {
        return Err(syn::Error::new_spanned(
            fields,
            "#[derive(Event)] only supports structs with named fields",
        ));
    };

    let mut topics = Vec::new();
    let mut payload = Vec::new();

    for field in &named.named {
        if field.attrs.iter().any(|a| a.path().is_ident("topic")) {
            topics.push(field);
        } else {
            payload.push(field);
        }
    }

    Ok((topics, payload))
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Event)] only supports structs",
        ));
    };

    let (topics, payload) = partition_fields(&data.fields)?;

    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) =
        input.generics.split_for_impl();

    // --- SIGNATURE const ---
    let type_name = ident.to_string();
    let version = std::env::var("CARGO_PKG_VERSION")
        .unwrap_or_else(|_| "0.0.0".to_string());
    let crate_name = std::env::var("CARGO_PKG_NAME")
        .unwrap_or_else(|_| "_unknown_".to_string());
    let sig_bytes = groundhog_contract_util::event_signature(
        &version,
        &crate_name,
        &type_name,
    );

    let sig_array = sig_bytes.iter().copied();

    // --- Event impl ---
    let num_topics = (topics.len() + 1) as u32; // +1 for SIGNATURE at index 0

    let topic_arms = topics.iter().enumerate().map(|(i, field)| {
        let index = (i + 1) as u32; // 0 is SIGNATURE
        let field_ident = field.ident.as_ref().unwrap();
        quote! {
            #index => ::core::option::Option::Some(
                self.#field_ident.into_topic()
            ),
        }
    });

    // --- WriteXDR impl ---
    let payload_writes = payload.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        quote! {
            ::xdrs::WriteXDR::write_xdr(&self.#field_ident, w)?;
        }
    });

    // --- Payload struct ---
    let payload_ident = format_ident!("{}Payload", ident);

    // Collect the payload fields (strip #[topic] attrs, keep everything else)
    let payload_fields: Vec<_> = payload.iter().map(|field| {
        let vis = &field.vis;
        let ident = &field.ident;
        let ty = &field.ty;
        // Keep non-topic attributes (e.g. #[allow(...)])
        let attrs: Vec<_> = field.attrs.iter()
            .filter(|a| !a.path().is_ident("topic"))
            .collect();
        quote! { #(#attrs)* #vis #ident: #ty }
    }).collect();

    // Copy lifetime params from the event struct for the payload struct.
    // Type params are not copied — payload fields shouldn't reference them.
    let lifetime_params: Vec<_> = input.generics.params.iter()
        .filter(|p| matches!(p, GenericParam::Lifetime(_)))
        .collect();

    let payload_generics = if lifetime_params.is_empty() {
        quote! {}
    } else {
        quote! { < #(#lifetime_params),* > }
    };

    // For the GAT assignment: map all lifetime params to '__payload
    let payload_lifetime_args: Vec<_> = input.generics.params.iter()
        .filter(|p| matches!(p, GenericParam::Lifetime(_)))
        .map(|_| quote! { '__payload })
        .collect();

    let payload_ty_in_gat = if payload_lifetime_args.is_empty() {
        quote! { #payload_ident }
    } else {
        quote! { #payload_ident < #(#payload_lifetime_args),* > }
    };

    Ok(quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            pub const SIGNATURE: &'static ::sdk::Topic = &[#(#sig_array),*];
        }

        #[derive(::xdrs::ReadXDR)]
        pub struct #payload_ident #payload_generics {
            #(#payload_fields),*
        }

        impl #impl_generics ::sdk::Event for #ident #ty_generics #where_clause {
            type Payload<'__payload> = #payload_ty_in_gat;

            fn num_topics(&self) -> u32 {
                #num_topics
            }

            fn topic<'__event>(
                &'__event self,
                index: u32,
            ) -> ::core::option::Option<::sdk::TopicSrc<'__event>> {
                match index {
                    0 => ::core::option::Option::Some(
                        ::sdk::TopicSrc::Shared(Self::SIGNATURE)
                    ),
                    #(#topic_arms)*
                    _ => ::core::option::Option::None,
                }
            }

            fn decode_payload<'__payload>(
                bytes: &'__payload [u8],
            ) -> ::core::result::Result<#payload_ty_in_gat, ::xdrs::ReadError> {
                ::xdrs::txs::decode(bytes)
            }
        }

        impl #impl_generics ::xdrs::WriteXDR for #ident #ty_generics #where_clause {
            fn write_xdr<__F, __W: ::xdrs::Writer<__F>>(
                &self,
                w: &mut __W,
            ) -> ::xdrs::WriteResult<()> {
                #(#payload_writes)*
                ::core::result::Result::Ok(())
            }
        }
    })
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p groundhog-macros`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add macros/src/derive_event.rs
git commit -m "feat: generate payload struct and decode_payload in derive(Event)"
```

---

### Task 3: Fix manual `Event` impls in `tests/events.rs`

**Files:**
- Modify: `tests/events.rs`

- [ ] **Step 1: Update `OwnedEvent` impl**

The `OwnedEvent` already derives `ReadXDR` and its `WriteXDR` serializes all fields (it's not using the derive macro). Its payload IS the full struct. Add to the `impl Event for OwnedEvent` block:

```rust
impl Event for OwnedEvent {
    type Payload<'a> = OwnedEvent;

    fn num_topics(&self) -> u32 {
        2
    }

    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => {
                let payer_payee =
                    (&self.payer, &self.payee).hash().into_inner();
                Some(TopicSrc::Owned(payer_payee))
            }
            1 => Some(TopicSrc::Owned(self.currency.hash().into_inner())),
            _ => None,
        }
    }

    fn decode_payload<'a>(bytes: &'a [u8]) -> Result<OwnedEvent, xdrs::ReadError> {
        decode_xdr(bytes)
    }
}
```

- [ ] **Step 2: Update `BorrowedEvent` impl**

The `BorrowedEvent` also serializes all fields via derive. Add to the `impl Event for BorrowedEvent` block:

```rust
impl<'a> Event for BorrowedEvent<'a> {
    type Payload<'p> = BorrowedEvent<'p>;

    fn num_topics(&self) -> u32 {
        3
    }

    fn topic<'t>(&'t self, index: u32) -> Option<TopicSrc<'t>> {
        match index {
            0 => return Some(TopicSrc::Shared(self.payer)),
            1 => return Some(TopicSrc::Shared(self.payee)),
            2 => {
                return Some(TopicSrc::Owned(
                    self.currency.hash().into_inner(),
                ));
            }
            _ => return None,
        }
    }

    fn decode_payload<'p>(bytes: &'p [u8]) -> Result<BorrowedEvent<'p>, xdrs::ReadError> {
        decode_xdr(bytes)
    }
}
```

- [ ] **Step 3: Verify tests compile and pass**

Run: `cargo test --features std --test events`
Expected: all 8 tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/events.rs
git commit -m "fix: update manual Event impls in tests with Payload GAT"
```

---

### Task 4: Fix manual `Event` impls in `native_exec/tests/fluent_api.rs`

**Files:**
- Modify: `native_exec/tests/fluent_api.rs`

- [ ] **Step 1: Update `TestValue` impl**

`TestValue` has 0 topics and its payload is itself (u32 wrapper). It already derives `ReadXDR`:

```rust
impl Event for TestValue {
    type Payload<'a> = TestValue;

    fn num_topics(&self) -> u32 {
        0
    }

    fn decode_payload<'a>(bytes: &'a [u8]) -> Result<TestValue, xdrs::ReadError> {
        xdrs::txs::decode(bytes)
    }
}
```

- [ ] **Step 2: Update `EventA` impl**

```rust
impl Event for EventA {
    type Payload<'a> = EventA;

    fn num_topics(&self) -> u32 {
        1
    }
    fn topic(&self, index: u32) -> Option<TopicSrc<'_>> {
        match index {
            0 => Some(TopicSrc::Owned(EVENT_A_SIG)),
            _ => None,
        }
    }

    fn decode_payload<'a>(bytes: &'a [u8]) -> Result<EventA, xdrs::ReadError> {
        xdrs::txs::decode(bytes)
    }
}
```

- [ ] **Step 3: Update `EventB` impl**

```rust
impl Event for EventB {
    type Payload<'a> = EventB;

    fn num_topics(&self) -> u32 {
        1
    }
    fn topic(&self, index: u32) -> Option<TopicSrc<'_>> {
        match index {
            0 => Some(TopicSrc::Owned(EVENT_B_SIG)),
            _ => None,
        }
    }

    fn decode_payload<'a>(bytes: &'a [u8]) -> Result<EventB, xdrs::ReadError> {
        xdrs::txs::decode(bytes)
    }
}
```

- [ ] **Step 4: Verify tests compile and pass**

Run: `cargo test -p groundhog-native-exec --test fluent_api`
Expected: all fluent_api tests pass.

- [ ] **Step 5: Commit**

```bash
git add native_exec/tests/fluent_api.rs
git commit -m "fix: update manual Event impls in fluent_api tests with Payload GAT"
```

---

### Task 5: Fix manual `Event` impls in ERC20 events

**Files:**
- Modify: `standard-interfaces/groundhog-erc20/src/events.rs`

- [ ] **Step 1: Add payload struct and update `TransferEvent` impl**

Add before the `impl Event for TransferEvent` block:

```rust
#[derive(ReadXDR)]
pub struct TransferEventPayload {
    pub amount: u128,
}
```

Update the `Event` impl:

```rust
impl<'a> Event for TransferEvent<'a> {
    type Payload<'p> = TransferEventPayload;

    fn num_topics(&self) -> u32 {
        3
    }

    fn topic(&self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => Some(TopicSrc::Shared(TRANSFER_EVENT_SIG)),
            1 => Some(TopicSrc::Shared(self.from)),
            2 => Some(TopicSrc::Shared(self.to)),
            _ => None,
        }
    }

    fn decode_payload<'p>(bytes: &'p [u8]) -> Result<TransferEventPayload, xdrs::ReadError> {
        sdk::internal::decode_xdr(bytes)
    }
}
```

- [ ] **Step 2: Add payload struct and update `ApprovalEvent` impl**

Add before the `impl Event for ApprovalEvent` block:

```rust
#[derive(ReadXDR)]
pub struct ApprovalEventPayload {
    pub difference: i128,
}
```

Update the `Event` impl:

```rust
impl<'a> Event for ApprovalEvent<'a> {
    type Payload<'p> = ApprovalEventPayload;

    fn num_topics(&self) -> u32 {
        3
    }

    fn topic(&self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => Some(TopicSrc::Shared(APPROVAL_EVENT_SIG)),
            1 => Some(TopicSrc::Shared(self.owner)),
            2 => Some(TopicSrc::Shared(self.spender)),
            _ => None,
        }
    }

    fn decode_payload<'p>(bytes: &'p [u8]) -> Result<ApprovalEventPayload, xdrs::ReadError> {
        sdk::internal::decode_xdr(bytes)
    }
}
```

- [ ] **Step 3: Add `ReadXDR` import if not present**

Check that `ReadXDR` is imported in the file. The file already imports `WriteXDR` from `sdk`. Add `ReadXDR` to the import if needed.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p groundhog-erc20-if`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add standard-interfaces/groundhog-erc20/src/events.rs
git commit -m "fix: update ERC20 Event impls with Payload GAT"
```

---

### Task 6: Add `assert_event_payload` to test infra

**Files:**
- Modify: `native_exec/src/frame_result.rs`
- Modify: `native_exec/src/transact_result.rs`

- [ ] **Step 1: Add `assert_event_payload` to `FrameResult`**

Add to `FrameResult` impl in `native_exec/src/frame_result.rs`, after the existing `assert_event_data` method:

```rust
    /// Assert on the decoded payload of an event, using the Event trait's
    /// associated Payload type. Strips topics automatically.
    pub fn assert_event_payload<E>(
        &self,
        index: usize,
        check: impl FnOnce(&E::Payload<'_>),
    ) where
        E: groundhog_event::Event,
    {
        let events = self.events();
        assert!(
            index < events.len(),
            "event index {} out of range (have {} events)",
            index,
            events.len()
        );
        let event = &events[index];
        let topics_len = 32 * event.num_topics as usize;
        assert!(
            topics_len <= event.serialized_event.len(),
            "event {} topics_len {} exceeds serialized_event length {}",
            index,
            topics_len,
            event.serialized_event.len()
        );
        let payload_bytes = &event.serialized_event[topics_len..];
        let decoded = E::decode_payload(payload_bytes)
            .unwrap_or_else(|e| panic!("failed to decode event payload: {}", e));
        check(&decoded);
    }
```

- [ ] **Step 2: Add delegation in `TransactResult`**

Add to `TransactResult` impl in `native_exec/src/transact_result.rs`, after the existing `assert_event_data` delegation:

```rust
    pub fn assert_event_payload<E>(
        &self,
        index: usize,
        check: impl FnOnce(&E::Payload<'_>),
    ) where
        E: groundhog_event::Event,
    {
        self.frame.assert_event_payload::<E>(index, check);
    }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p groundhog-native-exec`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add native_exec/src/frame_result.rs native_exec/src/transact_result.rs
git commit -m "feat: add assert_event_payload to test infra"
```

---

### Task 7: Integration tests using `assert_event_payload`

**Files:**
- Modify: `tests/events.rs`

- [ ] **Step 1: Update the fluent API test to use `assert_event_payload`**

Replace the manual payload decoding in `test_derived_event_fluent_api` with:

```rust
#[test]
fn test_derived_event_fluent_api() {
    let env = TestEnvironment::no_cost();

    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            let payer = ContractAddress::default();
            let payee = ContractAddress::default();
            log_event(&DerivedEvent {
                payer: &payer,
                payee: &payee,
                amount: 42,
                memo: b"hello",
            });
        })
    });

    result.assert_success();
    result.assert_event_count(1);

    // Check topics via raw event inspection
    result.assert_event(0, |e| {
        assert_eq!(e.num_topics, 3);

        let emitted = EmittedEvent::from(e);
        assert_eq!(emitted.topic(0).unwrap(), DerivedEvent::SIGNATURE);
        assert_eq!(emitted.topic(1).unwrap(), &*ContractAddress::default());
        assert_eq!(emitted.topic(2).unwrap(), &*ContractAddress::default());
    });

    // Check payload via the generated Payload type
    result.assert_event_payload::<DerivedEvent>(0, |p| {
        assert_eq!(p.amount, 42);
        assert_eq!(p.memo, b"hello");
    });
}
```

- [ ] **Step 2: Add a test for `SimpleEvent` payload decoding**

Add a new fluent API test:

```rust
#[test]
fn test_simple_event_payload_decoding() {
    let env = TestEnvironment::no_cost();

    let result = env.transact(|tx| {
        tx.call(&TEST_ADDR).run(|| {
            log_event(&SimpleEvent { value: 99 });
        })
    });

    result.assert_success();
    result.assert_event_count(1);

    result.assert_event_payload::<SimpleEvent>(0, |p| {
        assert_eq!(p.value, 99);
    });
}
```

- [ ] **Step 3: Run all event tests**

Run: `cargo test --features std --test events`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/events.rs
git commit -m "test: use assert_event_payload in integration tests"
```

---

### Task 8: Run full test suite

- [ ] **Step 1: Run all tests**

Run: `cargo test --features std --tests`
Expected: all tests pass across all crates.

- [ ] **Step 2: Run fluent_api tests**

Run: `cargo test -p groundhog-native-exec --test fluent_api`
Expected: all fluent_api tests pass.

- [ ] **Step 3: Check formatting**

Run: `cargo fmt --check`
Expected: no formatting issues.
