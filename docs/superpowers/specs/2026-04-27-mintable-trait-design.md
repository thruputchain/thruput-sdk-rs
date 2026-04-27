# Mintable Trait Design: Ergonomic Auth Token Minting & Consuming

## Summary

Add a `Mintable` trait and `UnsignedSigner<T>` type that let developers build and consume auth token trees as plain Rust structs. The `#[contracttype]` macro derives `Mintable` automatically, generating a companion "unsigned" type where `Signer<T>` fields become `UnsignedSigner<T::Unsigned>`. Minting recursively converts the unsigned struct into an `AuthTokenNode` tree. Consuming reverses the process.

**Key insight**: because `Signer<T>` serializes as zero bytes in XDR (see signer-token-design.md), the same `WriteXDR`/`ReadXDR` + `CollectAuthHandles`/`ReceiveAuthHandles` code paths work for both calldata and auth token payloads. The `Mintable` trait builds on this — it doesn't need a custom payload encoding.

## Protocol Types (from groundhog-xdr 0.9.2)

```rust
struct AuthTokenNode {
    consumer_address: ContractAddress,
    payload: AuthTokenPayload,           // opaque bytes, max 256
    child_nodes: VecM<AuthTokenNode, 4>, // max 4 children, max depth 4
}

struct AuthToken {
    mint_address: ContractAddress,       // filled by runtime from current address
    expiry_height: BlockHeight,          // passed as separate syscall param
    root_node: AuthTokenNode,            // deserialized from serialized_auth_token
}
```

## Syscalls

### MintAuthToken (number 33, exists)

```yaml
inputs:
  - expiry_height: BlockHeight
  - serialized_auth_token: span  # serialized AuthTokenNode
outputs:
  - token_id: u32
```

### ConsumeAuthToken (number 34, planned)

```yaml
inputs:
  - token_id: u32
outputs:
  - auth_payload: span
  - unwrapped_auth_tokens: span  # child token handles as [u32]
  - unwrapped_auth_token_written_len: RuntimeMemoryLength
  - unwrapped_auth_token_count: u32
```

Returns the payload of the consumed node plus new u32 handles for each child node.

## Design

### `UnsignedSigner<T>`

A named wrapper pairing a consumer address with unsigned data.

```rust
pub struct UnsignedSigner<T> {
    pub consumer: ContractAddress,
    pub data: T,
}
```

The unsigned form of a `Signer<T>`. Each `UnsignedSigner` becomes one `AuthTokenNode` in the tree.

### `Mintable` Trait

```rust
pub trait Mintable: Sized {
    type Unsigned;

    fn build_auth_token_node(unsigned: Self::Unsigned) -> AuthTokenNode;
}
```

- `Unsigned`: companion type for constructing values before minting.
- `build_auth_token_node`: recursively converts an unsigned value into an `AuthTokenNode`. Returns a node with a placeholder `consumer_address`; the `Signer<T>` impl sets the real consumer from `UnsignedSigner`.

No `from_consumed` method on the trait — consuming uses the standard `ReadXDR` + `ReceiveAuthHandles` code path (same as calldata decoding). See "Consuming" section below.

### Payload Encoding

Because `Signer<T>` serializes as zero bytes in XDR, the payload of each `AuthTokenNode` is just `WriteXDR` of the type:

- Non-Signer fields serialize normally
- `Signer<T>` fields contribute zero bytes
- `Option` discriminants and `Vec` lengths are preserved (structural metadata)
- Signer handles are **never** in the payload — they are runtime-local ephemeral IDs

Child nodes correspond to Signer fields by depth-first traversal order (same as `CollectAuthHandles`).

### Trait Implementations

#### Primitive types

For types with no Signer fields, unsigned form is the type itself. Leaf node.

```rust
impl Mintable for u64 {
    type Unsigned = u64;

    fn build_auth_token_node(unsigned: u64) -> AuthTokenNode {
        AuthTokenNode {
            consumer_address: ContractAddress::default(), // placeholder
            payload: xdr_encode(&unsigned),
            child_nodes: VecM::default(),
        }
    }
}
```

Generated via macro for all primitives, byte types, etc.

#### `Signer<T>`

```rust
impl<T: Mintable> Mintable for Signer<T> {
    type Unsigned = UnsignedSigner<<T as Mintable>::Unsigned>;

    fn build_auth_token_node(
        unsigned: UnsignedSigner<<T as Mintable>::Unsigned>,
    ) -> AuthTokenNode {
        let mut node = T::build_auth_token_node(unsigned.data);
        node.consumer_address = unsigned.consumer;
        node
    }
}
```

Builds the inner node, then sets consumer from `UnsignedSigner`. This is the only place consumer addresses are set.

#### `#[contracttype]` structs

For a struct like:

```rust
#[contracttype]
struct Transfer {
    amount: u64,
    auth: Signer<SubAuth>,
}
```

The macro generates:

```rust
mod __groundhog_mintable {
    pub struct TransferUnsigned {
        pub amount: <u64 as ::sdk::Mintable>::Unsigned,
        pub auth: <::sdk::Signer<SubAuth> as ::sdk::Mintable>::Unsigned,
    }
}

impl ::sdk::Mintable for Transfer {
    type Unsigned = __groundhog_mintable::TransferUnsigned;

    fn build_auth_token_node(unsigned: Self::Unsigned) -> ::sdk::AuthTokenNode {
        // 1. Build child AuthTokenNodes from Signer fields via CollectAuthHandles-like walk
        let auth_child = <::sdk::Signer<SubAuth> as ::sdk::Mintable>::build_auth_token_node(
            unsigned.auth,
        );

        // 2. Payload = WriteXDR of the struct (Signers write zero bytes)
        //    Construct a Transfer with the non-Signer values and a default Signer
        let payload_value = Transfer {
            amount: unsigned.amount,
            auth: ::sdk::Signer::default(),  // handle value is irrelevant — writes 0 bytes
        };
        let payload = ::sdk::xdr_encode(&payload_value);

        // 3. Assemble the node
        ::sdk::AuthTokenNode {
            consumer_address: ::sdk::ContractAddress::default(), // placeholder
            payload,
            child_nodes: ::sdk::auth_token_children(&[auth_child]),
        }
    }
}
```

The resolving types:
- `<u64 as Mintable>::Unsigned` = `u64`
- `<Signer<SubAuth> as Mintable>::Unsigned` = `UnsignedSigner<SubAuth>` (if SubAuth has no Signers)

Developer writes:

```rust
let unsigned = <Transfer as Mintable>::Unsigned {
    amount: 100,
    auth: UnsignedSigner {
        consumer: token_contract_addr,
        data: SubAuth { nonce: 42 },
    },
};
```

#### Types with no Signer fields

If all fields have `Unsigned = Self` (no direct Signer fields), the macro can set `type Unsigned = Self` and use standard `WriteXDR` directly. The companion struct is isomorphic anyway.

### `mint_auth_token` function

Takes an `UnsignedSigner` for uniform consumer handling:

```rust
pub fn mint_auth_token<T: Mintable>(
    expiry: BlockHeight,
    unsigned: UnsignedSigner<<T as Mintable>::Unsigned>,
) -> Signer<T> {
    let root_node = <Signer<T> as Mintable>::build_auth_token_node(unsigned);
    let serialized = xdr_encode_auth_token_node(&root_node);
    // Call MintAuthToken syscall with expiry + serialized AuthTokenNode
    // Returns Signer<T> wrapping the u32 handle
}
```

### Consuming

`Signer::consume` uses the same `ReadXDR` + `ReceiveAuthHandles` pattern as calldata decoding:

```rust
impl<T: ReadXDROwned + ReceiveAuthHandles> Signer<T> {
    pub fn consume(self) -> T {
        let (payload, child_handles) = syscall_consume(self.handle);
        let mut value: T = xdr_decode(&payload);
        let mut handles = child_handles.iter().copied();
        value.receive_auth_handles(&mut handles);
        value
    }
}
```

This is identical to how entrypoint codegen decodes calldata:
1. `ReadXDR` → struct with Signer handles uninitialized (zero bytes read)
2. `ReceiveAuthHandles` → fill in handles from side channel

No `Mintable::from_consumed` method needed — the standard traits handle it.

### Example: Nested Signers

```rust
#[contracttype]
struct Outer { x: u32, inner: Signer<Inner> }

#[contracttype]
struct Inner { y: u64, deep: Signer<Leaf> }

#[contracttype]
struct Leaf { data: u32 }
```

**Minting:**

```rust
let unsigned = UnsignedSigner {
    consumer: root_consumer,
    data: <Outer as Mintable>::Unsigned {
        x: 1,
        inner: UnsignedSigner {
            consumer: contract_a,
            data: <Inner as Mintable>::Unsigned {
                y: 2,
                deep: UnsignedSigner {
                    consumer: contract_b,
                    data: Leaf { data: 3 },
                },
            },
        },
    },
};

let signer: Signer<Outer> = mint_auth_token::<Outer>(expiry, unsigned);
```

Tree:

```
AuthTokenNode (consumer = root_consumer)
├── payload: WriteXDR(Outer { x: 1, inner: Signer(default) })
│            = just the bytes for x=1, inner contributes 0 bytes
└── child[0]: AuthTokenNode (consumer = contract_a)
    ├── payload: WriteXDR(Inner { y: 2, deep: Signer(default) })
    │            = just the bytes for y=2
    └── child[0]: AuthTokenNode (consumer = contract_b)
        ├── payload: WriteXDR(Leaf { data: 3 })
        └── children: []
```

**Consuming** (at root_consumer):

```rust
let outer: Outer = signer.consume();
// ReadXDR(payload) → Outer { x: 1, inner: Signer(uninit) }
// ReceiveAuthHandles → Outer { x: 1, inner: Signer(child_handle_0) }

let inner: Inner = outer.inner.consume();  // at contract_a
let leaf: Leaf = inner.deep.consume();     // at contract_b
```

### Constraints

- **Max depth**: 4 (`kAuthTokenMaxDepth`)
- **Max children per node**: 4 (`kAuthTokenMaxChildNodes`)
- **Max payload**: 256 bytes (`kAuthTokenMaxLen`)

Runtime limits, not compile-time.

### Changes Required

#### New types (`src/signer.rs`)
- `UnsignedSigner<T>` struct

#### `Mintable` trait + impls
- Trait definition with `Unsigned` + `build_auth_token_node`
- Primitive impls (macro), `Signer<T>` impl, `Option`, `Vec`, tuples
- `#[contracttype]` macro generates companion unsigned struct + `Mintable` impl

#### Updated `mint_auth_token`
- Takes `UnsignedSigner`, builds tree, serializes `AuthTokenNode`, calls syscall

#### `Signer::consume`
- Calls `ConsumeAuthToken` syscall, uses `ReadXDR` + `ReceiveAuthHandles`

#### `ReceiveAuthHandles` trait (from signer-token-design.md)
- Needed for both calldata decoding and consume — same trait, same impls

### Deferred Work

- **Compile-time depth/width checks**
- **`#[contracttype]` enum support for Mintable**
