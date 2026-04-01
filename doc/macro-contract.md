Expose methods for cross-contract calls.

This macro may be applied to:
- a trait
- a trait implementation
- an inherent implementation
- a free function

Usage examples are in the top-level [documentation](mod@crate).

#### Interface vs implementation

The easiest mental model is:
- Put `#[contract]` on a trait when you want to define a reusable interface.
- Put `#[contract]` on a trait impl when you want to export an implementation of
  that interface.
- Put `#[contract]` on an inherent impl when you want a self-contained contract
  with methods defined directly on the type.
- Put `#[contract]` on a free function when you only want an exported entrypoint
  and do not need a generated client.

#### Client objects

Generated clients default to the interface name plus `Client`:
- `#[contract(interface = "IERC20")] trait Token` generates `IERC20Client`
- `#[contract] trait Foo` generates `FooClient`
- `#[contract] impl Foo { ... }` generates `FooClient`

Use `client = CustomClient` only when you want to override that default.

For trait impls, the trait is authoritative for the client name. Adding an
explicit `client = ...` attribute to the impl is rejected to avoid
double-generation.

#### Contract exports

When `#[contract]` is applied to an implementation, its exported methods are
placed in a generated module:
- bare impls and free functions default to `exports`
- trait impls default to `<TraitName>_exports`

`exports = "custom_name"` overrides that default.

#### Errors

If the macro includes `throws = ErrorType`, the implementation may pass values
of that type to [`throw`]. `throws` accepts a single type path:

```rust,ignore
#[contract(throws = errors::MyError)]
```

or a list of type paths:

```rust,ignore
#[contract(throws(errors::AlphaError, errors::BetaError))]
```

`throws` belongs to the interface:
- use it on a trait definition
- or on an inherent impl
- not on a trait impl

Generated client error enums use the last path segment as the variant name. For
example, `throws(errors::MyError)` generates `ClientError::MyError(...)`.
Collisions like `foo::Error` and `bar::Error` are rejected at compile time.

Generated clients expose:
- `client.method(...)` for the success-oriented path
- `client.try_method(...) -> Result<_, ClientError>` for inspecting exceptions

#### Storage

If a method takes `self` or `&mut self`, the SDK reconstructs the contract state
from storage before calling the method.

If the receiver is `&mut self`, the updated state is written back only when the
method completes successfully. Exceptions and faults revert the frame.

The state type should derive [`Storage`](derive@crate::Storage) (or use
[`storage`](macro@crate::storage)).

Use `storage = false` for stateless contracts that should be called on a
temporary default value instead of persistent state.

#### Endpoint identifier customization

Each exported contract entrypoint is named with an identifier derived from:
- the interface name
- a version
- the method name

`interface = "InterfaceName"` overrides the interface-name portion.

For trait impls, repeat that `interface = "..."` on the impl when the trait
uses a custom interface name. The impl's exported entrypoint names are fixed
during macro expansion, so mismatches are rejected at compile time rather than
inherited automatically.

`version = "0.3.0"` overrides the version used for entrypoint identifiers. When
implementing a separately versioned trait interface, repeat that version on the
impl if you need the impl's entrypoint IDs to match the trait's explicit
version.
