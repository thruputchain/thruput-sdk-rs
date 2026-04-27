# Mintable Trait Design: Ergonomic Auth Token Minting

## Summary

Add a `Mintable` trait and `UnsignedSigner<T>` type that let developers build auth token trees as plain Rust structs, then mint them atomically. The `#[contracttype]` macro derives `Mintable` automatically, generating a companion "unsigned" type where `Signer<T>` fields become `UnsignedSigner<T::Unsigned>`. Minting recursively converts the unsigned struct into an `AuthTokenNode` tree and calls the `MintAuthToken` syscall once.

## Motivation

The `MintAuthToken` syscall expects a serialized `AuthTokenNode` (confirmed from the C++ runtime — `x23::deserialize(&auth_token.root_node, ...)` — the runtime wraps it with `mint_address` and `expiry_height` to form the full `AuthToken`). Each node has a consumer address, an opaque payload, and up to 4 child nodes. Manually constructing this tree is error-prone and leaks protocol internals into contract code.

The SDK should let developers work with typed structs. A struct with `Signer<T>` fields maps naturally to the tree: each `Signer` becomes a child `AuthTokenNode`, and the remaining non-signer fields become the payload.

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
}
```

- `Unsigned`: the companion type used to construct values before minting.
- `build_auth_token_node`: recursively converts an unsigned value into an `AuthTokenNode`. Returns a node with a placeholder `consumer_address` — the caller (`Signer<T>` impl or `mint_auth_token`) sets the real consumer.

### Trait Implementations

#### Primitive types

For types with no `Signer` fields, unsigned form is the type itself. The node is a leaf: payload is the XDR encoding of the value, no children.

```rust
impl Mintable for u64 {
    type Unsigned = u64;

    fn build_auth_token_node(unsigned: u64) -> AuthTokenNode {
        AuthTokenNode {
            consumer_address: ContractAddress::default(), // placeholder, set by caller
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

The `Signer<T>` impl builds the inner node via `T::build_auth_token_node`, then sets `consumer_address` from the `UnsignedSigner`. This is the only place where consumer addresses are set.

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
        // 1. Build child AuthTokenNodes from Signer fields
        let auth_child = <::sdk::Signer<SubAuth> as ::sdk::Mintable>::build_auth_token_node(
            unsigned.auth,
        );

        // 2. Serialize only non-Signer fields as the payload
        let payload = ::sdk::xdr_encode(&unsigned.amount);
        // (For multiple non-Signer fields, serialize as a tuple)

        // 3. Assemble the node
        ::sdk::AuthTokenNode {
            consumer_address: ::sdk::ContractAddress::default(), // placeholder
            payload,
            child_nodes: ::sdk::auth_token_children(&[auth_child]),
        }
    }
}
```

### Payload Encoding Strategy

The payload contains **only the non-Signer fields**, serialized as XDR. Signer handles are ephemeral runtime-local IDs (allocated from a bump allocator) and have no meaning outside the current execution context — they must never appear in the persisted `AuthTokenNode` tree.

The correspondence between payload fields and child nodes is positional: child node 0 maps to the first `Signer` field in declaration order, child node 1 to the second, etc. The type layout (known at compile time from the struct definition) provides this mapping.

**Encoding**: for a struct with fields `[F0, F1(Signer), F2, F3(Signer), F4]`:
- Payload = XDR of `(F0, F2, F4)` — non-Signer fields, in declaration order
- child_nodes[0] = AuthTokenNode for F1
- child_nodes[1] = AuthTokenNode for F3

**Decoding** (at consume time, future): reverse the process — deserialize non-Signer fields from payload, get child handles from runtime, interleave to reconstruct the full struct.

For types with no Signer fields, the payload is the standard `WriteXDR` of the whole struct (identical to normal XDR serialization).

The resolving unsigned types:
- `<u64 as Mintable>::Unsigned` = `u64` (primitive)
- `<Signer<SubAuth> as Mintable>::Unsigned` = `UnsignedSigner<SubAuth::Unsigned>` = `UnsignedSigner<SubAuth>` (if SubAuth has no Signers)

So the developer writes:

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

If a `#[contracttype]` struct has no `Signer` fields (directly), the macro can set `type Unsigned = Self` and `build_auth_token_node` serializes the whole struct as payload with no children.

A field might transitively contain Signers (e.g., field type `Foo` where `Foo` has a Signer member). The macro can't detect this syntactically. So it always uses `<FieldType as Mintable>::Unsigned` for each field type in the companion struct. If `FieldType::Unsigned == FieldType`, the companion struct is isomorphic — no harm.

### `mint_auth_token` function

The top-level minting entry point takes an `UnsignedSigner` so the root consumer is provided the same way as nested consumers:

```rust
pub fn mint_auth_token<T: Mintable>(
    expiry: BlockHeight,
    unsigned: UnsignedSigner<<T as Mintable>::Unsigned>,
) -> Signer<T> {
    let root_node = <Signer<T> as Mintable>::build_auth_token_node(unsigned);
    let serialized = xdr_encode(&root_node);
    // Call MintAuthToken syscall with expiry + serialized AuthTokenNode
    // Returns Signer<T> wrapping the u32 handle
}
```

Note: the current `mint_auth_token` in the codebase takes raw `&T` data. This replaces it with the Mintable-based version.

### Helper: `auth_token_children`

A convenience function to build a `VecM<AuthTokenNode, kAuthTokenMaxChildNodes>` without requiring `alloc`:

```rust
pub fn auth_token_children(nodes: &[AuthTokenNode]) -> VecM<AuthTokenNode, kAuthTokenMaxChildNodes> {
    nodes.try_into().expect("too many auth token children (max 4)")
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

Building the unsigned form:

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

This produces the tree:

```
AuthTokenNode (root, consumer = root_consumer)
├── payload: XDR(x: 1)     [only non-Signer fields]
└── child[0]: AuthTokenNode (consumer = contract_a)
    ├── payload: XDR(y: 2)  [only non-Signer fields]
    └── child[0]: AuthTokenNode (consumer = contract_b)
        ├── payload: XDR(data: 3)
        └── children: []
```

Tree depth = 3, within the `kAuthTokenMaxDepth` limit of 4.

### Constraints

- **Max depth**: 4 (`kAuthTokenMaxDepth`). Nesting more than 4 levels of `Signer<Signer<...>>` will fail at runtime.
- **Max children per node**: 4 (`kAuthTokenMaxChildNodes`). A struct with more than 4 `Signer` fields at one level will fail at runtime.
- **Max payload**: 256 bytes (`kAuthTokenMaxLen`). The XDR-encoded non-Signer fields of each individual node must fit.

These are runtime limits, not compile-time. The SDK could add compile-time checks later.

### Changes Required

#### New types and traits (`src/signer.rs`)
- `UnsignedSigner<T>` struct
- `Mintable` trait definition
- `Mintable` impl for `Signer<T>`
- Updated `mint_auth_token` function using the trait
- `auth_token_children` helper

#### `Mintable` impls for primitives (`src/signer.rs` or new file)
- Macro-generated impls for all primitive types, byte types, `Option<T>`, `Vec<T>`, tuples, etc.
- Pattern mirrors `CollectAuthHandles` impls

#### `#[contracttype]` macro changes (`macros/src/contract_type.rs`)
- Generate companion unsigned struct in a private `__groundhog_mintable` module
- Generate `Mintable` impl with `build_auth_token_node`
- Field-by-field: non-Signer fields serialize into payload, Signer fields become child nodes
- The payload serialization writes only non-Signer fields as XDR (not a standard `WriteXDR` of the struct)

#### Re-export `AuthTokenNode` from groundhog-xdr
- `src/types.rs`: add `pub use groundhog_xdr::auth_token::*;`

### Deferred Work

- **`consume()` syscall and reconstruction**: The inverse of minting — deserialize non-Signer fields from payload, get child handles from runtime, interleave to reconstruct the full struct. Will need a trait or macro-generated method for the positional interleaving.
- **Compile-time depth/width checks**: Static assertions that the Signer nesting depth doesn't exceed `kAuthTokenMaxDepth`.
- **`#[contracttype]` enum support for Mintable**: Enums with Signer-containing variants.
