# Signer Token SDK Support

## Summary

Add `Signer<T>` as a first-class type in the SDK, allowing contracts to mint, receive, consume, and forward opaque authentication tokens. Signers appear as normal method parameters in contract interfaces. The macro-generated code handles routing signer handle IDs through the `Invoke` syscall's auth token channel alongside regular calldata.

## Motivation

Contracts need to pass authentication tokens between each other during cross-contract calls. The runtime manages token ownership and lifetime (expiry). The SDK should make this ergonomic: a developer declares `Signer<T>` parameters in method signatures and the generated code handles the plumbing.

## Design

### The `Signer<T>` Type

```rust
use core::marker::PhantomData;

pub struct Signer<T> {
    handle: u32,
    _phantom: PhantomData<T>,
}
```

`T` is the auth data type associated with the token. The data lives in the runtime outside of wasm until explicitly consumed. `T` acts as a phantom type that constrains what `consume()` deserializes.

#### XDR Serialization

`Signer<T>` serializes as a plain `u32` in XDR — its handle value. This means signers travel in calldata alongside regular arguments. No special-casing in the XDR layer.

```rust
impl<T> WriteXDR for Signer<T> {
    // Writes the u32 handle
}

impl<T> ReadXDR for Signer<T> {
    // Reads a u32 handle, wraps in Signer
}
```

This means `Signer<T>` composes naturally with `Vec`, `Option`, nested structs, etc. — anywhere a `u32` can appear in XDR.

### The `CollectAuthHandles` Trait

A second trait extracts signer handle IDs from an argument tuple for the `Invoke` syscall's `owned_auth_token_ids_list` parameter. Regular types are no-ops; `Signer<T>` pushes its handle.

```rust
pub trait CollectAuthHandles {
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf);
}
```

Where `AuthHandleBuf` is a small fixed-capacity buffer of `u32` values (similar to `EncodeBuf` for calldata).

#### Implementations

**Primitive types** (`u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `u128`, `i128`, `bool`, etc.): no-op.

**`Signer<T>`**: pushes `self.handle`.

**Byte types** (`&[u8]`, `BytesM`, `VecM`, arrays): no-op.

**`Vec<T>`** / `VecM<T>` / slices: iterates elements, delegates to each.

**`Option<T>`**: delegates to inner value if `Some`, no-op if `None`.

**Tuples**: delegates to each element in order. Tuple impls are generated with a declarative macro, matching the existing XDR tuple impls.

**`#[contracttype]` structs and enums**: the `#[contracttype]` derive macro generates `CollectAuthHandles` alongside `WriteXDR`/`ReadXDR`, delegating to each field (structs) or the active variant's fields (enums).

#### Ordering Invariant

`CollectAuthHandles` traverses the data structure in the same depth-first order as XDR serialization. This means the handles in the auth token list correspond positionally to the `Signer` values as they appear in the serialized calldata. Both sides (caller and callee) agree on ordering because they share the same type definitions.

### Syscall Wrappers

#### `mint_auth_token`

Wraps the `MintAuthToken` syscall (number 33).

```rust
pub fn mint_auth_token<T: WriteXDR>(
    expiry: BlockHeight,
    data: &T,
) -> Signer<T> {
    let serialized = /* XDR-encode data */;
    let handle = sys::MintAuthToken(expiry, &serialized);
    Signer { handle, _phantom: PhantomData }
}
```

#### `Signer::consume`

Wraps a future consume syscall (not yet defined in the protocol). Retrieves the serialized auth data from the runtime and deserializes it.

```rust
impl<T: ReadXDROwned> Signer<T> {
    pub fn consume(self) -> T {
        let bytes = /* syscall: consume token by handle, get serialized data */;
        /* XDR-decode bytes as T */
    }
}
```

This is deferred until the consume syscall is added to the protocol.

### Cross-Contract Call Flow

#### Caller Side (Client Codegen)

The generated client method:

1. Encodes all arguments (including signers) as XDR into calldata. `Signer<T>` writes its `u32` handle as part of normal serialization.
2. Collects auth handles from all arguments via `CollectAuthHandles` into a `[u32]` buffer.
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

1. Fetches calldata from the VM via `GetCalldata` (ignoring the second auth token output — pass empty slice).
2. Decodes all arguments from calldata via `ReadXDR`. `Signer<T>` reads its `u32` handle normally.
3. Calls the method implementation with the decoded arguments.

No second pass is needed on the receiving side. The handle values are in the calldata.

```rust
// Generated entrypoint (simplified)
fn transfer_internal() {
    let calldata = get_calldata();
    let (amount, auth): (u64, Signer<TransferAuth>) =
        contract_call_decode(calldata.as_ref()).unwrap();
    state.transfer(amount, auth);
}
```

### Forwarding

A `Signer<T>` received as a method parameter can be passed directly to another contract's method — it's just a value with a `u32` handle. The generated client code for the downstream call collects its handle via `CollectAuthHandles` and passes it through `Invoke`, transferring runtime ownership.

No special `forward()` method is needed.

### Composition

Because `Signer<T>` implements `WriteXDR`, `ReadXDR`, and `CollectAuthHandles`, it composes naturally in all positions:

**Intermixed parameters:**
```rust
fn transfer(&self, amount: u64, auth: Signer<TransferAuth>, memo: &str)
```

**Inside structs:**
```rust
#[contracttype]
struct BatchTransfer {
    amount: u64,
    auth: Signer<TransferAuth>,
}
```

**Inside collections:**
```rust
fn batch(&self, transfers: Vec<Signer<TransferAuth>>)
```

**Optional:**
```rust
fn maybe_auth(&self, auth: Option<Signer<TransferAuth>>)
```

In all cases, the XDR layer handles structural metadata (lengths, discriminants) and the `CollectAuthHandles` traversal extracts handles in matching order.

### Changes Required

#### `Signer<T>` type definition
- New module in the SDK (e.g., `src/signer.rs`)
- `Signer<T>` struct with `WriteXDR`, `ReadXDR` impls (as `u32`)
- `CollectAuthHandles` impl

#### `CollectAuthHandles` trait + impls
- Trait definition with `AuthHandleBuf` type
- Impls for all primitive types, byte types, `Vec`, `Option`, tuples
- Tuple impls via declarative macro (matching existing XDR pattern)

#### `#[contracttype]` macro changes
- Derive `CollectAuthHandles` alongside XDR traits
- Structs: delegate to each field in declaration order
- Enums: delegate to the active variant's fields

#### `mint_auth_token` wrapper
- New function wrapping `MintAuthToken` syscall

#### `contract_call` changes (`src/call/calls.rs`)
- Accept auth handle buffer from caller
- Pass auth handles to `raw_Invoke` (replacing the current `&[]`)
- Wire `InvokeConfig.flags` through (replacing the current `0`)

#### Client codegen changes (`gen/src/client.rs`)
- After encoding args, call `collect_auth_handles` on the args tuple
- Pass both buffers through the call chain

#### `InvokeConfig` changes
- No structural changes needed; signers flow through method args, not config

### Deferred Work

- **Consume syscall**: not yet in the protocol. `Signer::consume()` will be added when available.
- **`GetCalldata` second output**: the auth token ID list output from `GetCalldata` is ignored for now (pass empty slice). May be useful for future validation or introspection.
- **Spec generation**: `Signer<T>` will need representation in the contract spec format so that off-chain tooling can distinguish signer parameters from regular `u32` values.
