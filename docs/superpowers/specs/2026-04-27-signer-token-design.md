# Signer Token SDK Support

## Summary

Add `Signer<T>` as a first-class type in the SDK, allowing contracts to mint, receive, consume, and forward opaque authentication tokens. Signers appear as normal method parameters in contract interfaces. `Signer<T>` serializes as **zero bytes** in XDR — handles travel out-of-band via the auth token channel (for cross-contract calls) or the child handle list (for consume). This unified serialization means the same XDR format works for both calldata and auth token payloads.

## Motivation

Contracts need to pass authentication tokens between each other during cross-contract calls. The runtime manages token ownership and lifetime (expiry). The SDK should make this ergonomic: a developer declares `Signer<T>` parameters in method signatures and the generated code handles the plumbing.

## Design

### The `Signer<T>` Type

```rust
use core::marker::PhantomData;

#[derive(Clone, Copy, ReadXDR, WriteXDR)]
pub struct Signer<T> {
    handle: u32,
    _phantom: PhantomData<T>,
}
```

`T` is the auth data type associated with the token. The data lives in the runtime outside of wasm until explicitly consumed. `T` acts as a phantom type that constrains what `consume()` deserializes.

The `handle` field is a runtime-local ephemeral ID (bump-allocated by the runtime). It is meaningful only during the current execution and must never be persisted or serialized.

#### XDR Serialization: Zero Bytes

`Signer<T>` serializes as **zero bytes** in XDR. The `handle` field is excluded from serialization; `PhantomData<T>` derives to `()`. This means:

- In **calldata**, Signer values contribute nothing to the byte stream. Handles are passed separately via the auth token ID list in the `Invoke` syscall.
- In **auth token payloads**, Signer values contribute nothing. Child handles are returned separately by the `ConsumeAuthToken` syscall.

Both contexts use the same `WriteXDR`/`ReadXDR` encoding, and both fill in handles via a second pass (`ReceiveAuthHandles`).

### `CollectAuthHandles` Trait (Sending Side)

Extracts signer handle IDs from a value for the `Invoke` syscall's `owned_auth_token_ids_list` parameter. Regular types are no-ops; `Signer<T>` pushes its handle.

```rust
pub trait CollectAuthHandles {
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf);
}
```

Where `AuthHandleBuf` is a small fixed-capacity buffer of `u32` values.

#### Implementations

- **Primitive types**: no-op
- **`Signer<T>`**: pushes `self.handle`
- **Byte types** (`&[u8]`, `BytesM`, arrays): no-op
- **`Vec<T>`** / `VecM<T>` / slices: iterates, delegates to each element
- **`Option<T>`**: delegates if `Some`, no-op if `None`
- **Tuples**: delegates to each element in order (macro-generated, up to 14 elements)
- **`#[contracttype]` structs/enums**: macro-generated, delegates to each field/variant

#### Ordering Invariant

`CollectAuthHandles` traverses the data structure in the same depth-first order as XDR serialization. This means the handles in the auth token list correspond positionally to the `Signer` values as they appear in the type's structural walk. Both caller and callee agree on ordering because they share the same type definitions.

### `ReceiveAuthHandles` Trait (Receiving Side)

The inverse of `CollectAuthHandles`. Walks a deserialized value and fills in Signer handles from an iterator. Used after `ReadXDR` to populate the handles that were not in the byte stream.

```rust
pub trait ReceiveAuthHandles {
    fn receive_auth_handles(&mut self, handles: &mut impl Iterator<Item = u32>);
}
```

#### Implementations

- **Primitive types**: no-op
- **`Signer<T>`**: `self.handle = handles.next().expect("missing handle")`
- **Byte types**: no-op
- **`Vec<T>`** / `VecM<T>` / slices: iterates, delegates to each element
- **`Option<T>`**: delegates if `Some`, no-op if `None`
- **Tuples**: delegates to each element in order
- **`#[contracttype]` structs/enums**: macro-generated, delegates to each field/variant

#### Ordering Invariant

`ReceiveAuthHandles` traverses in the same depth-first order as `CollectAuthHandles` and XDR serialization. Handles are consumed greedily in declaration order. `Option::None` and empty `Vec` consume zero handles; `Option::Some` and each `Vec` element consume handles according to their inner type.

### Cross-Contract Call Flow

#### Caller Side (Client Codegen)

The generated client method:

1. Encodes all arguments as XDR into calldata. `Signer<T>` writes zero bytes.
2. Collects auth handles via `CollectAuthHandles` into a `[u32]` buffer.
3. Passes both the calldata buffer and the auth handle buffer to `raw_Invoke`.

```rust
// Generated client method (simplified)
pub fn transfer(&self, amount: u64, auth: Signer<TransferAuth>) {
    let args = (amount, auth);
    let calldata = contract_call_encode(&args);
    let mut auth_handles = AuthHandleBuf::new();
    args.collect_auth_handles(&mut auth_handles);
    // raw_Invoke(..., &calldata, flags, &auth_handles, ...)
}
```

#### Callee Side (Entrypoint Codegen)

The generated entrypoint:

1. Fetches calldata and auth token ID list from `GetCalldata` (both outputs).
2. Decodes all arguments from calldata via `ReadXDR`. `Signer<T>` reads zero bytes (handle field is left as 0).
3. Fills in Signer handles from the auth token ID list via `ReceiveAuthHandles`.
4. Calls the method implementation with the fully populated arguments.

```rust
// Generated entrypoint (simplified)
fn transfer_internal() {
    let (calldata, auth_token_ids) = get_calldata_with_auth_tokens();
    let mut args: (u64, Signer<TransferAuth>) =
        contract_call_decode(calldata.as_ref()).unwrap();
    let mut handles = auth_token_ids.iter().copied();
    args.receive_auth_handles(&mut handles);
    state.transfer(args.0, args.1);
}
```

### Forwarding

A `Signer<T>` received as a method parameter can be passed directly to another contract's method. The generated client code collects its handle via `CollectAuthHandles` and passes it through `Invoke`, transferring runtime ownership. No special method needed.

### Composition

Because `Signer<T>` implements `WriteXDR` (zero bytes), `ReadXDR` (zero bytes), `CollectAuthHandles`, and `ReceiveAuthHandles`, it composes naturally in all positions:

```rust
fn transfer(&self, amount: u64, auth: Signer<TransferAuth>, memo: &str)
```

```rust
#[contracttype]
struct BatchTransfer {
    amount: u64,
    auth: Signer<TransferAuth>,
}
```

```rust
fn batch(&self, transfers: Vec<Signer<TransferAuth>>)
fn maybe_auth(&self, auth: Option<Signer<TransferAuth>>)
```

In all cases, XDR handles structural metadata (lengths, discriminants). `CollectAuthHandles`/`ReceiveAuthHandles` handle the Signer values, traversing in the same depth-first order. `Option::None` and empty `Vec` naturally consume zero handles.

### Unified Serialization with Auth Token Payloads

The same zero-byte XDR encoding means auth token payloads (inside `AuthTokenNode`) use the exact same format as calldata. When minting, `WriteXDR` produces the payload (Signers contribute nothing); `CollectAuthHandles` identifies child nodes. When consuming, `ReadXDR` decodes the payload; `ReceiveAuthHandles` fills in handles from child tokens.

No separate "payload encoding" is needed — it's the same code path as calldata.

### Changes Required

#### `Signer<T>` type definition
- `src/signer.rs`: struct with `WriteXDR`/`ReadXDR` deriving to zero bytes (handle excluded, PhantomData → ())
- `CollectAuthHandles` impl (pushes handle)
- `ReceiveAuthHandles` impl (pops handle)

#### `CollectAuthHandles` trait + impls
- Trait definition with `AuthHandleBuf` type
- Impls for all primitive types, byte types, `Vec`, `Option`, tuples
- Tuple impls via declarative macro

#### `ReceiveAuthHandles` trait + impls
- Trait definition (mirror of `CollectAuthHandles`)
- Same impl coverage: primitives (no-op), `Signer` (pop handle), containers (delegate), tuples (macro)

#### `#[contracttype]` macro changes
- Derive `CollectAuthHandles` and `ReceiveAuthHandles` alongside XDR traits

#### `contract_call` changes (`src/call/calls.rs`)
- Pass auth handles to `raw_Invoke`

#### Client codegen changes (`gen/src/client.rs`)
- Collect auth handles from args tuple, pass to call

#### Entrypoint codegen changes (`gen/src/exports.rs`)
- Use `GetCalldata` second output (auth token ID list)
- After `ReadXDR` decode, call `ReceiveAuthHandles` to fill in Signer handles

#### `mint_auth_token` wrapper
- Wraps `MintAuthToken` syscall using `WriteXDR` (zero-byte signers) for payload + `CollectAuthHandles` for child node construction

#### `Signer::consume` method
- Wraps `ConsumeAuthToken` syscall: `ReadXDR` for payload + `ReceiveAuthHandles` with child handles

### Deferred Work

- **Spec generation**: `Signer<T>` will need representation in the contract spec format so that off-chain tooling can distinguish signer parameters.
- **Compile-time depth/width checks**: Static assertions for auth token tree limits.
