# `#[derive(Event)]` Macro Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate event boilerplate by deriving `Event` and `WriteXDR` impls from a struct with `#[topic]` field annotations.

**Architecture:** A derive macro in `groundhog-macros` generates three things: an associated `SIGNATURE` const (compile-time SHA-256 of `major::crate::TypeName`), an `Event` trait impl routing topics through `IntoTopic`, and a `WriteXDR` impl serializing only non-topic fields. The `IntoTopic` trait lives in `groundhog-event`; hash-based impls live in the `sdk` crate using the VM's SHA-256 syscall.

**Tech Stack:** `syn`, `quote`, `proc-macro2` (already in `groundhog-macros`), `sha2` (already a dep of `groundhog-macros` and `contract-util`)

**Spec:** `docs/superpowers/specs/2026-03-27-derive-event-macro.md`

---

### Task 1: `event_signature` function in `contract-util`

**Files:**
- Modify: `contract-util/src/lib.rs`
- Modify: `contract-util/src/tests.rs`

- [ ] **Step 1: Write the failing tests**

Add to `contract-util/src/tests.rs`:

```rust
#[test]
fn test_event_signature_deterministic() {
    let s1 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
    let s2 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
    assert_eq!(s1, s2);
}

#[test]
fn test_event_signature_is_32_bytes() {
    let sig = crate::event_signature("1.0.0", "mycrate", "MyEvent");
    assert_eq!(sig.len(), 32);
}

#[test]
fn test_event_signature_same_major_same_signature() {
    let s1 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
    let s2 = crate::event_signature("1.9.0", "mycrate", "MyEvent");
    assert_eq!(s1, s2);
}

#[test]
fn test_event_signature_differs_by_version() {
    let s1 = crate::event_signature("1.0.0", "mycrate", "MyEvent");
    let s2 = crate::event_signature("2.0.0", "mycrate", "MyEvent");
    assert_ne!(s1, s2);
}

#[test]
fn test_event_signature_differs_by_crate() {
    let s1 = crate::event_signature("1.0.0", "crate_a", "MyEvent");
    let s2 = crate::event_signature("1.0.0", "crate_b", "MyEvent");
    assert_ne!(s1, s2);
}

#[test]
fn test_event_signature_differs_by_name() {
    let s1 = crate::event_signature("1.0.0", "mycrate", "AlphaEvent");
    let s2 = crate::event_signature("1.0.0", "mycrate", "BetaEvent");
    assert_ne!(s1, s2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p groundhog-contract-util`
Expected: compilation error — `event_signature` not found.

- [ ] **Step 3: Implement `event_signature`**

Add to `contract-util/src/lib.rs` after the `error_selector` function:

```rust
/// Derive a standard event signature for a contract event type.
///
/// Event signature = sha256(MAJOR_VERSION :: CRATE_NAME :: TYPE_NAME)
///
/// Uses the same hashing scheme as `error_selector` but returns the
/// full 32-byte hash instead of truncating to u64.
pub fn event_signature(
    version_str: &str,
    crate_name: &str,
    type_name: &str,
) -> [u8; 32] {
    let major_version = semver::major_version_from_string(version_str);

    let mut hasher = sha2::Sha256::new();
    hasher.update(major_version.as_bytes());
    hasher.update(b"::");
    hasher.update(crate_name.as_bytes());
    hasher.update(b"::");
    hasher.update(type_name.as_bytes());
    hasher.finalize().into()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p groundhog-contract-util`
Expected: all tests pass, including the six new ones.

- [ ] **Step 5: Commit**

```bash
git add contract-util/src/lib.rs contract-util/src/tests.rs
git commit -m "feat: add event_signature function to contract-util"
```

---

### Task 2: `IntoTopic` trait in `groundhog-event`

**Files:**
- Modify: `groundhog-event/src/lib.rs`

- [ ] **Step 1: Add `IntoTopic` trait and `[u8; 32]` impl**

Add to `groundhog-event/src/lib.rs` after the `Event` trait:

```rust
/// Trait for types that can be converted into an event topic.
///
/// Types that are already 32 bytes (like `ContractAddress`) implement
/// this as a zero-copy borrow. Other types should XDR-encode and hash
/// to produce a 32-byte topic.
pub trait IntoTopic {
    fn into_topic(&self) -> TopicSrc<'_>;
}

impl IntoTopic for [u8; 32] {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Shared(self)
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p groundhog-event`
Expected: compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add groundhog-event/src/lib.rs
git commit -m "feat: add IntoTopic trait to groundhog-event"
```

---

### Task 3: Re-export `IntoTopic` from the SDK and add hash-based impls

**Files:**
- Modify: `src/event.rs`

- [ ] **Step 1: Add re-export and hash-based helper**

In `src/event.rs`, add to the re-export line and add the helper plus impls:

Change the re-export line from:
```rust
pub use groundhog_event::{Event, Topic, TopicSrc};
```
to:
```rust
pub use groundhog_event::{Event, IntoTopic, Topic, TopicSrc};
```

Then add after the re-exports:

```rust
/// Convert any `WriteXDR` value into a topic by XDR-encoding and hashing.
pub fn hash_to_topic<T: crate::WriteXDR>(val: &T) -> Topic {
    let buf = crate::internal::encode_xdr(val).unwrap();
    crate::crypto::sha256(buf.as_ref()).into_inner()
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p sdk --features std`
Expected: compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add src/event.rs
git commit -m "feat: re-export IntoTopic and add hash_to_topic helper"
```

---

### Task 4: `#[derive(Event)]` macro

**Files:**
- Create: `macros/src/derive_event.rs`
- Modify: `macros/src/lib.rs`

- [ ] **Step 1: Add the derive macro entry point**

Add to `macros/src/lib.rs`:

```rust
mod derive_event;
```

(Add after the existing `mod derive_exn;` line.)

And add the derive macro function:

```rust
#[proc_macro_derive(Event, attributes(topic))]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    derive_event::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
```

- [ ] **Step 2: Implement `derive_event::expand`**

Create `macros/src/derive_event.rs`:

```rust
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, Result};

/// Returns (topic_fields, payload_fields) where each entry is (field_ident, field_index).
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
                ::sdk::IntoTopic::into_topic(&self.#field_ident)
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

    Ok(quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            pub const SIGNATURE: &'static ::sdk::Topic = &[#(#sig_array),*];
        }

        impl #impl_generics ::sdk::Event for #ident #ty_generics #where_clause {
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

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p groundhog-macros`
Expected: compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add macros/src/derive_event.rs macros/src/lib.rs
git commit -m "feat: implement #[derive(Event)] macro"
```

---

### Task 5: Re-export `Event` derive from SDK

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Add the re-export**

In `src/lib.rs`, change the macro re-exports. After the existing line:

```rust
pub use groundhog_macros::{hash_to_array, sha256_static, static_key};
```

Add:

```rust
pub use groundhog_macros::Event;
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p sdk --features std`
Expected: compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add src/lib.rs
git commit -m "feat: re-export Event derive macro from sdk"
```

---

### Task 6: Integration test

**Files:**
- Modify: `tests/events.rs`

- [ ] **Step 1: Write the test**

Add to the end of `tests/events.rs`:

```rust
//-------------------------------------------------------------------------
// An event using #[derive(Event)]

#[derive(Event)]
struct DerivedEvent<'a> {
    #[topic]
    payer: &'a ContractAddress,
    #[topic]
    payee: &'a ContractAddress,
    amount: u128,
    memo: &'a [u8],
}

single_event_test! {
    fn test_derived_event() {
        let payer = ContractAddress::default();
        let payee = ContractAddress::default();
        let event = DerivedEvent {
            payer: &payer,
            payee: &payee,
            amount: 42,
            memo: b"hello",
        };

        log_event(&event);
    }
    fn event_check(event) {
        // 3 topics: signature + payer + payee
        assert_eq!(event.num_topics, 3);

        // Topic 0 is the SIGNATURE
        let emitted = EmittedEvent::from(event);
        assert_eq!(emitted.topic(0).unwrap(), DerivedEvent::SIGNATURE);

        // Topics 1 and 2 are the payer/payee addresses
        assert_eq!(
            emitted.topic(1).unwrap(),
            &ContractAddress::default()
        );
        assert_eq!(
            emitted.topic(2).unwrap(),
            &ContractAddress::default()
        );

        // Payload is only amount + memo (not the topic fields)
        let payload = event_payload(event);

        // Decode just the amount and memo
        #[derive(Debug, PartialEq, ReadXDR)]
        struct Payload<'a> {
            amount: u128,
            memo: &'a [u8],
        }

        let decoded: Payload = decode_xdr(payload).unwrap();
        assert_eq!(decoded.amount, 42);
        assert_eq!(decoded.memo, b"hello");
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test --features std test_derived_event -- --nocapture`
Expected: PASS. The derived event emits 3 topics (signature + 2 addresses) and a payload containing only `amount` and `memo`.

- [ ] **Step 3: Write a test for a zero-topic event (signature only)**

Add to `tests/events.rs`:

```rust
#[derive(Event)]
struct SimpleEvent {
    value: u128,
}

single_event_test! {
    fn test_derived_event_no_topics() {
        let event = SimpleEvent { value: 99 };
        log_event(&event);
    }
    fn event_check(event) {
        // 1 topic: just the signature
        assert_eq!(event.num_topics, 1);

        let emitted = EmittedEvent::from(event);
        assert_eq!(emitted.topic(0).unwrap(), SimpleEvent::SIGNATURE);

        let payload = event_payload(event);
        let decoded: u128 = decode_xdr(payload).unwrap();
        assert_eq!(decoded, 99);
    }
}
```

- [ ] **Step 4: Run all event tests**

Run: `cargo test --features std events -- --nocapture`
Expected: all event tests pass, including the existing ones (no regressions).

- [ ] **Step 5: Commit**

```bash
git add tests/events.rs
git commit -m "test: add integration tests for #[derive(Event)]"
```

---

### Task 7: Verify ERC20 events could use the new macro (manual check)

This is a verification task — no code changes. Confirm the macro would produce equivalent behavior for `TransferEvent`.

- [ ] **Step 1: Compare generated output**

Mentally verify that `#[derive(Event)]` on:

```rust
#[derive(Event)]
pub struct TransferEvent<'a> {
    #[topic]
    pub from: &'a ContractAddress,
    #[topic]
    pub to: &'a ContractAddress,
    pub amount: u128,
}
```

would generate impls equivalent to the hand-written ones in `standard-interfaces/groundhog-erc20/src/events.rs`:

1. `WriteXDR` — only serializes `amount` ✓
2. `Event::num_topics()` — returns 3 ✓
3. `Event::topic(0)` — returns signature (note: the existing `TRANSFER_EVENT_SIG` uses `hash_to_array!("TransferEvent")` which hashes just the name; the new macro hashes `major::crate::TransferEvent` — this is an **intentional change** that adds version/crate namespacing)
4. `Event::topic(1)` / `topic(2)` — returns `from`/`to` via `IntoTopic` which returns `TopicSrc::Shared` for `[u8; 32]` ✓

- [ ] **Step 2: Document the signature change**

The existing `TRANSFER_EVENT_SIG` uses `hash_to_array!("TransferEvent")` — a plain hash of the name string. The new `SIGNATURE` const uses `sha256("major::crate::TransferEvent")`. This is intentional: new events use the namespaced scheme. Existing events can migrate when ready. No action needed now — just awareness.
