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

The macro reserves `new`, `with_gas_limit`, `with_commit_locking`,
`with_read_only`, and the `try_` prefix for method/function identifiers on
exported contract surfaces. `#[contract]`
rejects trait methods, trait-impl methods, public inherent-impl methods, and
free `#[contract] fn` names that use them to avoid collisions with generated
client helpers and keep naming consistent across contract surfaces.

#### Contract exports

The generated exports module is an internal implementation detail. The
`exports = "custom_name"` attribute only overrides that internal module name
when you need a disambiguating name during macro expansion; it is not a public
API surface. Free-function contracts still support `exports = ...`, but they do
not generate a client or `<Interface>Entrypoints` wrapper.

Generated entrypoint metadata lives on `<Interface>Entrypoints`:
- trait definitions generate the wrapper alongside the client
- bare impls generate the wrapper together with the exports module

For bare impls, the default interface name and domain separator are derived from
the self type name.

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

`throws` declares the interface's error contract: it is published in the
contract spec and documents what callers should expect. It does not restrict
what a caller can catch — exceptions are identified on the wire by a selector
derived from the type, so a caller can match any exception type it knows
about, declared or not. Two types sharing a last path segment (`foo::Error`
and `bar::Error`) are fine.

Generated clients expose:
- `client.method(...)` for the success-oriented path
- `client.try_method(...) -> Result<_, CallError>` for inspecting exceptions,
  matched with [`match_exn!`](crate::match_exn)

#### Storage

If a method takes `self` or `&mut self`, the SDK reconstructs the contract state
from storage before calling the method.

If the receiver is `&mut self`, the updated state is written back only when the
method completes successfully. Exceptions and faults revert the frame.

The state type should derive [`Storage`](derive@crate::Storage) (or use
[`storage`](macro@crate::storage)).

#### Endpoint identifier customization

Each exported contract entrypoint is named with an identifier derived from:
- the interface name
- a version
- the method name

`interface = "InterfaceName"` overrides the interface-name portion.

For bare impls, omitting `interface = "..."` uses the self type name for this
metadata by default.

For trait impls, a plain `#[contract]` inherits the interface name, full
version, and entrypoint IDs from the trait, including across crates and through
trait aliases or re-exports. The implementing crate's version does not affect
that interface's identity. There is no need to repeat `interface` or `version`
on the impl.

`version = "0.3.0"` overrides the version on a trait definition, bare impl, or
free function; otherwise those declarations use their crate's own version.
Entrypoint IDs use the semver major line (including the minor component for
`0.x` versions).

`version` is not allowed on trait impls, even if it matches the trait. Remove
the argument to inherit the trait's version.

For compatibility, an explicit `interface` argument on a trait impl is accepted
as an assertion that the name matches the trait. A mismatch is rejected at
compile time; the trait remains authoritative.
