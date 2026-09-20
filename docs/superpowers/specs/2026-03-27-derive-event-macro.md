# `#[derive(Event)]` Macro Design Spec

This document records the original macro design. The `IntoTopic` encoding guidance
below is updated for direct scalar topics; the other generated-code sketches are historical.

## Overview

A derive macro that eliminates boilerplate for contract event types. Given a struct with `#[topic]` annotations, it generates the `Event` trait impl, a `WriteXDR` impl (payload-only), and a compile-time event signature constant.

## User-Facing API

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

- `#[derive(Event)]` on a struct triggers all codegen.
- `#[topic]` marks fields that become indexed topics.
- Remaining fields become the XDR-serialized payload.
- Structs with lifetime parameters are supported.

## Generated Code

For the example above, the macro generates:

```rust
impl<'a> TransferEvent<'a> {
    pub const SIGNATURE: &'static Topic = &[/* sha256("1::groundhog_erc20::TransferEvent") */];
}

impl<'a> Event for TransferEvent<'a> {
    fn num_topics(&self) -> u32 {
        3 // 1 signature + 2 #[topic] fields
    }

    fn topic(&self, index: u32) -> Option<TopicSrc<'a>> {
        match index {
            0 => Some(TopicSrc::Shared(Self::SIGNATURE)),
            1 => Some(self.from.into_topic()),
            2 => Some(self.to.into_topic()),
            _ => None,
        }
    }
}

impl<'a> WriteXDR for TransferEvent<'a> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.amount.write_xdr(w)?;
        Ok(())
    }
}
```

### Signature Constant

- `SIGNATURE` is an associated const on the struct: `&'static Topic`.
- Computed at compile time by the proc macro using `sha256("major::crate::TypeName")`.
- Uses the same `major_version :: crate_name :: type_name` format as `error_selector`.

### Topic Count Enforcement

- The protocol allows up to 3 topics (including the signature topic), leaving room for 2 `#[topic]` fields.
- This limit is **not** enforced at compile time.
- `log_event` returns an error at runtime if the limit is exceeded.

## `IntoTopic` Trait

### Definition (in `sdk::event`)

```rust
pub trait IntoTopic {
    fn into_topic(&self) -> TopicSrc<'_>;
}
```

Returns `TopicSrc` directly: `Shared` borrows existing topic bytes, while `Owned`
holds a constructed 32-byte value, either directly encoded or hashed.

### Identity impl (in `sdk::event`)

```rust
impl IntoTopic for [u8; 32] {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Shared(self)
    }
}
```

`Topic` is an alias for `[u8; 32]`. `ContractAddress` and `SHA256Hash` have explicit
implementations that delegate to their underlying 32-byte arrays. References
delegate through `impl<T: IntoTopic + ?Sized> IntoTopic for &T`.

### Direct scalar impls (in `sdk` crate)

Supported scalar topics use a fixed 32-byte big-endian representation:

| Types | Encoding |
|---|---|
| `u32`, `u64`, `u128`, `U256` | Unsigned value, zero-extended on the left |
| `i32`, `i64`, `i128`, `I256` | Two's-complement value, sign-extended on the left |
| `bool` | Zero-extended `0` or `1` |

Other primitive integer types, including `u8`, `u16`, `i8` and `i16`, do not
implement `IntoTopic`. Topic encoding is not the scalar's XDR serialization.
For example, `42u64` is 31 zero bytes followed by `0x2a`; `-2i64` is 31 `0xff`
bytes followed by `0xfe`.

```rust
impl IntoTopic for u128 {
    fn into_topic(&self) -> TopicSrc<'_> {
        let value: U256 = (*self).into();
        TopicSrc::Owned(value.to_be_bytes())
    }
}
```

### Hash-based impls (in `sdk` crate)

`str` and `[u8]` slices are always XDR-encoded and SHA-256 hashed, even when an
individual value is shorter than 32 bytes. This keeps the representation consistent
for each type rather than switching between direct bytes and hashes by value length.

```rust
impl IntoTopic for str {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Owned(hash_to_topic(&self))
    }
}
```

These implementations live in `sdk` because `crypto::sha256` is a VM syscall.
The `hash_to_topic` helper retains explicit serialization-and-hashing behavior.

Users can implement `IntoTopic` for their own types.

## `event_signature` Function (in `contract-util`)

```rust
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

Parallel to `error_selector` but returns the full 32-byte hash instead of truncating to `u64`. Uses `sha2` which is already a build dependency of the macro crate.

## Crate Placement

| Component | Crate | Rationale |
|---|---|---|
| `IntoTopic` trait | `sdk::event` | Owns implementations for primitive types alongside hashing support |
| `IntoTopic` impl for `[u8; 32]` | `sdk::event` | Identity conversion, no hashing needed |
| `IntoTopic` scalar impls | `sdk::event` | Reuse `U256`/`I256` widening and big-endian encoding |
| `IntoTopic` hash-based impls | `sdk` | Need `sdk::crypto::sha256` (VM syscall) |
| `event_signature` function | `contract-util` | Compile-time hashing, parallel to `error_selector` |
| `#[derive(Event)]` macro | `groundhog-macros` | Alongside existing `#[exn]`, `#[derive(Storage)]` |
| Re-export of `IntoTopic` | `sdk` root | User-facing convenience |

## What the Macro Does NOT Generate

- No convenience `log()` method — use `log_event(&my_event)`.
- No `TryFrom<EmittedEvent>` for parsing — hand-written when needed.
- No `Display` impl.
- No compile-time topic count enforcement.
