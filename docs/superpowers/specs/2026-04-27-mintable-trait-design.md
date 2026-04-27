# Mintable Trait Design: Ergonomic Auth Token Minting & Consuming

## Summary

Add a `Mintable` trait, `UnsignedSigner<T>` type, and `Signer::consume()` method that let developers build and consume auth token trees as plain Rust structs. The `#[contracttype]` macro derives `Mintable` automatically, generating a companion "unsigned" type where `Signer<T>` fields become `UnsignedSigner<T::Unsigned>`. Minting recursively converts the unsigned struct into an `AuthTokenNode` tree and calls the `MintAuthToken` syscall. Consuming reverses the process via the `ConsumeAuthToken` syscall.

## Motivation

The `MintAuthToken` syscall expects a serialized `AuthTokenNode` (confirmed from the runtime C++ — `x23::deserialize(&auth_token.root_node, ...)` — the runtime wraps it with `mint_address` and `expiry_height` to form the full `AuthToken`). Each node has a consumer address, an opaque payload, and up to 4 child nodes. Manually constructing this tree is error-prone and leaks protocol internals into contract code.

The SDK should let developers work with typed structs. A struct with `Signer<T>` fields maps naturally to the tree: each `Signer` becomes a child `AuthTokenNode`, and the remaining fields become the payload.

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
  - auth_payload: span           # the consumed node's payload bytes
  - unwrapped_auth_tokens: span  # child token handles as [u32]
  - unwrapped_auth_token_written_len: RuntimeMemoryLength
  - unwrapped_auth_token_count: u32
```

Returns the payload of the consumed node plus new u32 handles for each child node. The child nodes remain in the runtime until consumed individually.

## Design

### `UnsignedSigner<T>`

A named wrapper pairing a consumer address with unsigned data. Lives in `src/signer.rs` alongside `Signer<T>`.

```rust
pub struct UnsignedSigner<T> {
    pub consumer: ContractAddress,
    pub data: T,
}
```

This is the unsigned form of a `Signer<T>`. When minting, each `UnsignedSigner` becomes one `AuthTokenNode` in the tree.

### `Mintable` Trait

```rust
pub trait Mintable: Sized {
    type Unsigned;

    fn build_auth_token_node(unsigned: Self::Unsigned) -> AuthTokenNode;

    fn from_consumed(payload: &[u8], child_handles: &mut impl Iterator<Item = u32>) -> Self;
}
```

- `Unsigned`: the companion type used to construct values before minting.
- `build_auth_token_node`: recursively converts an unsigned value into an `AuthTokenNode`. Returns a node with a placeholder `consumer_address` — the `Signer<T>` impl sets the real consumer from `UnsignedSigner`.
- `from_consumed`: reconstructs the type from a consumed node's payload bytes and a stream of child handles. Greedy: each Signer slot pops the next handle from the iterator.

### Trait Implementations

#### Primitive types

For types with no `Signer` fields, unsigned form is the type itself. The node is a leaf (payload = XDR of value, no children). Consuming reads the full payload.

```rust
impl Mintable for u64 {
    type Unsigned = u64;

    fn build_auth_token_node(unsigned: u64) -> AuthTokenNode {
        AuthTokenNode {
            consumer_address: ContractAddress::default(),
            payload: xdr_encode(&unsigned),
            child_nodes: VecM::default(),
        }
    }

    fn from_consumed(payload: &[u8], _handles: &mut impl Iterator<Item = u32>) -> Self {
        xdr_decode(payload)
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

    fn from_consumed(_payload: &[u8], handles: &mut impl Iterator<Item = u32>) -> Self {
        Signer::from_handle(handles.next().expect("missing child handle"))
    }
}
```

At mint: builds the inner node, then sets consumer from `UnsignedSigner`.

At consume: pops the next child handle from the iterator. The `Signer<T>` is now a live handle that can be consumed again later to get the inner `T`.

#### `Option<Signer<T>>` and `Vec<Signer<T>>`

These work naturally with the greedy iterator approach.

**`Option<T>` where T: Mintable:**

```rust
impl<T: Mintable> Mintable for Option<T> {
    type Unsigned = Option<T::Unsigned>;

    fn build_auth_token_node(unsigned: Option<T::Unsigned>) -> AuthTokenNode {
        // Payload: discriminant (0 or 1) + inner payload if Some
        // Children: inner's children if Some, none if None
    }

    fn from_consumed(payload: &[u8], handles: &mut impl Iterator<Item = u32>) -> Self {
        // Read discriminant from payload
        // If Some: T::from_consumed(remaining_payload, handles)
        // If None: None (no handles consumed)
    }
}
```

**`Vec<T>` / slices where T: Mintable:**

At mint: payload contains the length, then each element's non-signer data. Each element's Signer fields become child nodes.

At consume: read length from payload, then reconstruct each element via `T::from_consumed`, which greedily pops handles as needed.

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
        // Walk fields in declaration order:
        // - amount (non-Signer): XDR-encode into payload
        // - auth (Signer): build child AuthTokenNode

        let auth_child = <::sdk::Signer<SubAuth> as ::sdk::Mintable>::build_auth_token_node(
            unsigned.auth,
        );

        let payload = ::sdk::xdr_encode(&unsigned.amount);

        ::sdk::AuthTokenNode {
            consumer_address: ::sdk::ContractAddress::default(),
            payload,
            child_nodes: ::sdk::auth_token_children(&[auth_child]),
        }
    }

    fn from_consumed(
        payload: &[u8],
        handles: &mut impl Iterator<Item = u32>,
    ) -> Self {
        // Walk fields in declaration order:
        // - amount (non-Signer): XDR-decode from payload
        // - auth (Signer): pop next handle

        let mut reader = ::sdk::xdr_reader(payload);
        let amount = u64::xdr_read(&mut reader);
        let auth = <::sdk::Signer<SubAuth>>::from_consumed(&[], handles);

        Transfer { amount, auth }
    }
}
```

### Payload Encoding Strategy

The payload is a **structural walk** of the type where:

- **Non-Signer fields**: serialized as XDR inline (standard `WriteXDR`)
- **Signer fields**: contribute 0 bytes to payload; become child nodes instead
- **`Option<T>` containing Signers**: discriminant written to payload; if Some, inner T's non-signer parts go in payload, inner T's signers go to children
- **`Vec<T>` containing Signers**: length written to payload; each element's non-signer parts go in payload, signers go to children

Signer handles are ephemeral runtime-local IDs (allocated from a bump allocator) and must never appear in the persisted `AuthTokenNode` tree.

The correspondence between child nodes and Signer fields is **greedy positional**: child handles are consumed in the order that Signer slots are encountered during a depth-first walk of the type. This is deterministic from the type layout.

**Example**: struct with fields `[x: u32, opt: Option<Signer<A>>, y: i64, items: Vec<Signer<B>>]`:

Mint (assuming opt=Some, items has 2 elements):
- Payload bytes: `XDR(x) || discriminant(1) || XDR(y) || length(2)`
- child_nodes: `[A_node, B_node_0, B_node_1]`

Consume:
- Read `x` from payload → u32
- Read discriminant=1 → Some, pop handle → `Some(Signer(h0))`
- Read `y` from payload → i64
- Read length=2, pop 2 handles → `vec![Signer(h1), Signer(h2)]`

### `mint_auth_token` function

The top-level minting entry point takes an `UnsignedSigner` so the root consumer is provided the same way as nested consumers:

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

### `Signer::consume` method

```rust
impl<T: Mintable> Signer<T> {
    pub fn consume(self) -> T {
        // Call ConsumeAuthToken syscall with self.handle
        // Returns: payload bytes + child handle list
        let (payload, child_handles) = syscall_consume(self.handle);
        let mut handles = child_handles.iter().copied();
        T::from_consumed(&payload, &mut handles)
    }
}
```

### Example: Nested Signers

```rust
#[contracttype]
struct Outer {
    x: u32,
    inner: Signer<Inner>,
}

#[contracttype]
struct Inner {
    y: u64,
    deep: Signer<Leaf>,
}

#[contracttype]
struct Leaf {
    data: u32,
}
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

Produces tree:

```
AuthTokenNode (consumer = root_consumer)
├── payload: XDR(x=1)
└── child[0]: AuthTokenNode (consumer = contract_a)
    ├── payload: XDR(y=2)
    └── child[0]: AuthTokenNode (consumer = contract_b)
        ├── payload: XDR(data=3)
        └── children: []
```

**Consuming** (at root_consumer's contract):

```rust
let outer: Outer = signer.consume();
// outer.x == 1
// outer.inner is a Signer<Inner> with a live handle

let inner: Inner = outer.inner.consume();  // at contract_a
// inner.y == 2
// inner.deep is a Signer<Leaf> with a live handle

let leaf: Leaf = inner.deep.consume();     // at contract_b
// leaf.data == 3
```

### Constraints

- **Max depth**: 4 (`kAuthTokenMaxDepth`). Nesting more than 4 levels of `Signer` will fail at runtime.
- **Max children per node**: 4 (`kAuthTokenMaxChildNodes`). A struct with more than 4 Signer fields at one level will fail at runtime. `Vec<Signer<T>>` with >4 elements will also fail.
- **Max payload**: 256 bytes (`kAuthTokenMaxLen`). The XDR-encoded non-Signer data of each individual node must fit.

These are runtime limits, not compile-time. The SDK could add compile-time checks later.

### Changes Required

#### New types and traits (`src/signer.rs` or `src/mintable.rs`)
- `UnsignedSigner<T>` struct
- `Mintable` trait definition (with `build_auth_token_node` and `from_consumed`)
- `Mintable` impl for `Signer<T>`
- Updated `mint_auth_token` function using the trait
- `Signer::consume` method (once ConsumeAuthToken syscall is available)

#### `Mintable` impls for primitives
- Macro-generated impls for all primitive types, byte types, `Option<T>`, `Vec<T>`, tuples
- Pattern mirrors `CollectAuthHandles` impls

#### `#[contracttype]` macro changes (`macros/src/contract_type.rs`)
- Generate companion unsigned struct in a private `__groundhog_mintable` module
- Generate `Mintable` impl with `build_auth_token_node` and `from_consumed`
- Field-by-field walk: non-Signer fields serialize/deserialize inline, Signer fields become/consume child nodes

#### Re-export auth token types from groundhog-xdr
- `src/types.rs`: add `pub use groundhog_xdr::auth_token::*;`

### Deferred Work

- **Compile-time depth/width checks**: Static assertions that the Signer nesting depth doesn't exceed `kAuthTokenMaxDepth`.
- **`#[contracttype]` enum support for Mintable**: Enums with Signer-containing variants.
