This SDK facilitates writing Groundhog contracts in Rust.

A contract is an ordinary Rust crate. Its persistent state lives in one or
more structs deriving [`Storage`](derive@crate::Storage); its behavior lives
in one or more impl or trait blocks marked
[`#[contract]`](macro@crate::contract), which generate the guest entrypoints
and a typed client that other contracts and tests call through.

```rust,ignore
use sdk::storage::Value;
use sdk::*;

#[derive(Storage)]
pub struct MyContract {
    contract_owner: Value<ContractAddress>,
}

#[contract]
impl MyContract {
    pub fn initialize(&mut self, contract_owner: &ContractAddress) {
        self.contract_owner.set(contract_owner);
    }

    pub fn get_owner(&self) -> ContractAddress {
        self.contract_owner.get()
    }
}
```

`#[derive(Storage)]` can also be spelled
[`#[storage]`](macro@crate::storage).

That expands to a `MyContractClient` for callers, guest entrypoints for the
deployed artifact, and a contract spec describing the interface. Declaring
exceptions is a separate step, covered under
[calling and errors](#calling-and-errors).

Sections: [Cargo features](#cargo-features), [storage](#storage),
[calling and errors](#calling-and-errors), [events](#events),
[authorization](#authorization), [arithmetic](#arithmetic),
[system functions](#system-functions),
[interfaces and specs](#interfaces-and-specs), and [testing](#testing).

# Cargo features

The features are layered: each implies the one above it. A contract
targeting `riscv32im-unknown-none-elf` normally enables none of them; the
host-side test build enables `dev`.

- **(none)** — the default, and what a deployed contract is built with:
  `no_std`, no allocator, no host dependencies. [`mod@storage`], events,
  exceptions, authorization and the syscall wrappers all work here. Only the
  allocating and host-side APIs need more.
- **`alloc`** — links the [`alloc`] crate, making `Vec`, `String`, and other
  allocating types usable in contract code and in storage. See
  [Allocation](#allocation) for what backs it on a guest.
- **`std`** — implies `alloc`. Adds the interfaces that need `std`, owned
  (allocating) contract-spec models, and detailed XDR decode errors. Host-only:
  enabling `std` for a `riscv32` build is a hard compile error, as is omitting
  it for a native build.
- **`dev`** — implies `std`. The native mock executor and everything that
  hangs off it: [`TestEnvironment`], [`client_test`], [`test_rv32()`],
  [`declare_mock_contract!`], and the storage-cost helpers. A contract crate
  forwards this from its own `dev` feature. It is the only feature that pulls
  `groundhog-native-exec`, and through it the hogd FFI library, into the
  dependency graph. That is why it is a feature: crates that only read
  contract specs build `sdk` without it and never fetch that library.
- **`logging`** — trace contract calls during native tests. Not implied by
  `dev`; enable it when you want the trace.
- **`source-build`** — compile the cmake source-build path into
  `groundhog-sys`. It does not select that path: `HOGD_FFI_BUILD=1` does, and
  it needs this feature to have something to select. Set both to build the
  hogd FFI library from source rather than download a prebuilt one, which is
  also the only way to build on a platform with no prebuilt binary. Only
  meaningful alongside `dev`.

A contract crate wires them up like this:

```toml
[features]
# Yours, not the SDK's. `#[contract]` gates guest entrypoint codegen on it,
# so a dependency cannot inject entrypoints into your binary.
entrypoints = []
dev = ["sdk/dev"]

[lib]
crate-type = ["rlib"]

# The rv32 artifact. `groundhog build -p my-contract` derives
# `--bin my-contract` from the package name, so this name must match it.
# src/main.rs is a `#![no_std] #![no_main]` wrapper whose `extern crate
# my_contract;` force-links the rlib so the entrypoint records reach the
# binary.
[[bin]]
name = "my-contract"
path = "src/main.rs"
required-features = ["entrypoints"]

[dependencies]
sdk = { workspace = true }

[dev-dependencies]
my-contract = { path = ".", features = ["dev"] }
```

Two things that look like features are keyed on the target instead:

- **Diagnostics.** The `Display` impls on exception types and the formatting
  they need are host-only. Off a Cargo feature, no dependency can turn them
  on for a deployed contract, so they cannot change what a contract computes.
- **The guest runtime.** `#[global_allocator]`, `#[panic_handler]`, `_start`
  and entrypoint dispatch come from `groundhog-rv32-runtime`, force-linked on
  `riscv32` and absent from host builds.

# Allocation

### Static

Static allocations are facilitated in contracts by [`ContractCell`] and
[`contract_local!`].

### Heap

With feature `alloc`:

- The [`alloc`] crate is linked in, giving access to allocating datatypes like
  `String` and `Vec`.
- A simple bump allocator is installed by the guest runtime crate
  (`groundhog-rv32-runtime`). Allocations are not freed until the current
  contract invocation terminates.

Datatypes defined by third-party libraries or by the user may be used in memory
and in persistent storage, noting that:

- Heap allocation requires feature `alloc`.
- Persistent storage fields must use SDK storage types or nested structs derived
  with [`Storage`](derive@crate::Storage).

# Storage

The [`mod@storage`] module provides a high-level layer over persistent state,
including collection types like [`Map`](storage::Map) and
[`Vector`](storage::Vector). It is available in every feature configuration.

Important storage-model rules:

- Plain data in persistent state must be wrapped in [`Value<T>`](storage::Value).
- [`Map<K, V>`](storage::Map) entries are individually addressed and may be
  absent.
- [`Vector<S>`](storage::Vector) is sparse-capable.
  [`Vector::len()`](storage::Vector::len) reports the in-bounds extent;
  individual slots may still be empty if their storage has been deleted.
- Use [`Vector::push_with()`](storage::Vector::push_with) for the common
  append-and-initialize flow, and
  [`Vector::push_empty_slot()`](storage::Vector::push_empty_slot) only when you
  intentionally want to expose the new slot before initializing it.
- [`Value::load_mut()`](storage::Value::load_mut) returns an auto-flushing edit
  guard: the updated value is written back when the guard is dropped.
- Nested storage structs inside collections must be wrapped in
  [`Node<T>`](storage::Node), for example `Vector<Node<MyFields>>`.

Two types enforce their invariant in the storage layer instead of in contract
code: [`NNInt`](storage::NNInt) for counters that must not go negative, and
[`Balance`](storage::Balance), whose `increase`/`decrease` fail on overflow
and overdraft in every build profile (see [Arithmetic](#arithmetic)). This
matters most under concurrency: when two transactions in the same block both
move a balance, the check has to hold against the committed value, not the
one the contract read.
[`Deletable`](derive@crate::Deletable) derives recursive deletion for a
storage struct.

Each contract's state occupies its own storage domain. `#[contract]` generates
a per-crate test asserting the crate's own roots do not collide; a binary that
links several contract crates should also invoke
[`storage_domain_uniqueness_test!`] so the aggregate is checked.

The low-level syscall wrappers ([`bytestring_set`] and friends) will corrupt
this module's bookkeeping if pointed at the same keys. Use them, with
[`static_key!`](macro@crate::static_key), only for state that
[`mod@storage`] does not own.

# Contract macros

The [`storage`](macro@crate::storage) macro is shorthand for
`#[derive(Storage)]`.

The [`contract`](macro@crate::contract) macro exports contract methods for
cross-contract calls and also generates client types for calling those methods.
It applies to a trait, a trait impl, an inherent impl, or a free function —
see its own page for the interface-versus-implementation split, `throws`, and
entrypoint-identifier customization.

Client naming defaults:

- `#[contract] trait Foo` generates `FooClient`
- `#[contract] impl Foo { ... }` generates `FooClient`
- `client = CustomClient` is an override, not a requirement

# Calling And Errors

[`ContractClient`] is the low-level call surface. Most code goes through a
generated client, which exposes two calling styles:

- `client.method(...)`: success-oriented path. Typed exceptions are re-thrown to
  the caller, and faults stay faults.
- `client.try_method(...)`: inspection path. Returns
  `Result<ReturnType, CallError>`. A caught exception stays undecoded: match
  it with [`match_exn!`], or decode it as a named type with `try_as` /
  `try_decode`.

```rust,ignore
match token.try_transfer(&to, amount) {
    Ok(()) => { /* ... */ }
    Err(CallError::Fault(f)) => { /* ... */ }
    Err(e) => match_exn!(e, {
        ERC20Error::TooLarge               => { /* ... */ }
        LibraryExn::NonNegativeIntOverflow => { /* ... */ }
        _ => e.re_throw(),
    }),
}
```

Everything structural — `Ok`, `Fault`, `Invalid` — stays an ordinary Rust
`match` with real exhaustiveness checking; the macro's only job is typed
downcasting. See [`match_exn!`] for the matching rules.

Use [`InvokeConfig`] (or the generated `with_gas_limit` constructor) to bound
the gas a subcall may spend, and [`deploy`] to create new contracts.
[`get_deploy_address`] computes the resulting address ahead of time.

At the frame level, contract execution ends in one of three states:

- Success: return data is produced and the frame commits.
- Exception: the frame reverts with a structured payload ([`throw`], library
  exception, or forwarded exception).
- Fault: the frame reverts without a structured exception payload
  (for example `panic!`).

Only success commits the current invocation frame's storage changes.

### Exceptions

A throwable type is one annotated with
[`#[contract_exception]`](macro@crate::contract_exception) (or
[`#[exn]`](macro@crate::exn) if you write your own derives), which implements
[`Exn`] and gives the type a selector derived from its canonical signature.
Clients dispatch on that selector, so a caller can tell your exceptions apart
from an unrelated contract's without agreeing on discriminant numbering.
Declare which ones a contract may throw with `throws = ...` on `#[contract]`.

The SDK's own failures arrive as [`LibraryExn`]. Absence (a missing key, an
empty queue) is an expected outcome, and comes back the same way; identify it
with [`is_storage_missing`](LibraryExn::is_storage_missing), or use
[`none_if_missing`](NoneIfMissing::none_if_missing) to narrow just that case
to `None` while re-throwing everything else.

A `try_*` function never aborts the VM on its own account: its syscalls,
buffer fetches and decoding all report failure through the return type, so the
only way one aborts is a closure you handed it. [`mod@exn`] states this in
full.

# Events

Emit with [`mod@event`]; derive the type with
[`#[derive(Event)]`](derive@crate::Event).

Derived events reserve topic 0 for the event signature. That signature is based
on the package name, package version, and event type name, so changing any of
those changes the emitted signature topic.

Groundhog currently supports up to three total topics per event. With
`#[derive(Event)]`, that means the signature plus up to two additional
`#[topic]` fields.

Types that are not already 32 bytes wide are XDR-encoded and hashed before
being used as topics. That hashing is one-way, so no reader can reconstruct a
whole event from the log: it sees raw topics plus the decoded non-topic
payload. That is what [`TypedEvent`] holds, and what the harness queries
[`assert_event`](TransactResult::assert_event) and
[`filter_events`](TransactResult::filter_events) hand back.

# Authorization

Authorization travels with the call. A [`Signed<T>`] parameter is a handle to
a host-held auth token: it encodes to zero bytes, and the handle rides
alongside the XDR arguments. A method signature therefore states in its types
which arguments must have been authorized.

[`Signed::consume`] hands back the payload and the address that minted the
token. Any contract can mint a token whose payload decodes as `T`, so
consuming proves only that some contract minted it and that this contract is
its intended consumer. **Check the returned minter against an address you
trust before acting on the payload.** Skipping that check is an authorization
bypass.

- [`#[contract_type]`](macro@crate::contract_type) makes a struct or enum
  usable as an argument, return value, or auth-token payload. It generates
  the auth-handle walkers that let a `Signed<T>` nest inside it (an enum
  variant included), and the `Mintable` preimage mirror used to mint
  tokens.
- [`mod@signer`] holds the token vocabulary: `Signed<Opaque>` for a token whose
  payload type is not yet known, `Signed::assume` to reinterpret one, and
  `AssumeTokens` to destructure a `Vec<Signed<Opaque>>` into a typed tuple.
- [`mod@auth_claim`] reads the current transaction's `AuthClaim` and, for a
  contract acting as an account, declares the public keys and weights its
  address requires.

`groundhog-signed-op` is the wallet-facing shape of this — Groundhog's
ERC-1271 equivalent, with the message typed and domain-separated in the
spirit of EIP-712 and replay-protected by the chain's uniquifier model.
`execute_signed_op` returns the one auth token a wallet mints — the root of
whatever it authorizes, with tokens for downstream contracts as child nodes.
The caller names its payload type with `Signed::assume`, then consumes it
and checks its minter against the wallet address it trusts.

### Minting limits

A token node carries at most **4** direct children
(`groundhog_xdr::auth_token::kAuthTokenMaxChildNodes`), and [`mint_auth_token`]
**panics** past that rather than reporting it. The count is per node and per
level of the preimage — containers do not open a new level, so five elements
in a `Vec<Signed<u64>>` field are five children of one node. A nested
`Signed<T>` does open a level: its own payload gets its own budget of four.

This matters more on chain than in a test: a panicking contract faults with
no message, code or location ([#358]), so a preimage that outgrew the cap is
indistinguishable from any other trap. Keep preimage shapes within the cap by
construction.

The encoded payload is separately capped at 256 bytes
(`kAuthTokenMaxLen`), which *is* reported — as
`AuthTokenError::MintEncodeFailure`.

[#358]: https://github.com/groundhogchain/sdk_rs/issues/358

# Arithmetic

[`U256`] and its signed counterpart [`I256`] behave like `u128` and `i128`,
including on overflow: the operators wrap when `overflow-checks` is off — as
in every contract profile — and panic when it is on. **The native test harness
builds with overflow checks on**, so an overflow that fails a test wraps
silently on-chain. A passing test proves nothing about overflow behavior in
deployment.

Signed division is the one exception, and it follows the primitives: `I256::MIN
/ -1` and `I256::MIN % -1` panic in *every* profile, because that is what
`i128` does — the hardware alternative is a trap, so Rust does not gate it on
`overflow-checks`. Use `wrapping_div` / `wrapping_rem` if you need the wrap.

Both types store eight 32-bit limbs, which is what the rv32 guest multiplies
natively, and [`I256`] is the same bits read signed rather than a second
implementation. Prefer them over a general-purpose big-integer crate: those
typically use 64-bit limbs and multiply through `u128`, which the guest has to
synthesize, so they cost far more code and gas for the same arithmetic.

For money math, be explicit rather than relying on operators:

- `checked_add` / `checked_sub` / `checked_mul` on the primitives and on
  [`U256`], with a typed throw on `None`;
- [`mul_div_u128`] / [`checked_mul_div_u128`] for `a * b / d` with a 256-bit
  intermediate (the panics of [`mul_div_u128`] are unconditional, not
  `overflow-checks`-gated);
- [`U256::mul_div`] when the operands are themselves 256-bit — full
  precision, on an exact 512-bit intermediate;
- the storage-level guards [`NNInt`](storage::NNInt) and
  [`Balance`](storage::Balance), which check in every profile.

Beyond that, [`U256`] and [`I256`] carry OpenZeppelin's `Math.sol` and
`SignedMath.sol` surfaces — `mul_shr`, `mul_mod`, `mod_exp`, `inv_mod`,
`average`, `ceil_div`, and `integer_sqrt`/`log2`/`log10`/`log256` with a
[`Rounding`] direction — so a Solidity port has the same vocabulary. Every
fallible one ships as a `checked_` form beside the panicking one.

# System functions

The SDK provides convenient wrappers around Groundhog syscalls:

- [`deploy`]: deploy other contracts
- [`ContractClient`]: call other contracts (typically via clients generated by
  [`contract`](macro@crate::contract))
- [`contract_op`]: contract entrance and exit routines
- [`crypto`]: hash and signature operations
- [`Hasher`]: incremental hashing
- Key-value storage primitives, re-exported at the crate root:
  - [`delete`], [`delete_exclusive`]: remove entries
  - [`bytestring_set`], [`bytestring_get_sized`]: byte blobs
  - [`non_negative_int_set`], [`non_negative_int_inc`]: non-negative integers
  - [`retrocausal_queue_insert`], [`retrocausal_queue_consume_sized`]:
    retrocausal queues
- Low-level syscall bindings:
  - [`mod@sys`]
- Emitting events:
  - [`mod@event`]

Context about the running transaction:

- [`mod@block`]: the height of the block being executed in
- [`mod@stats`]: transaction statistics, including
  [`gas_remaining()`](stats::gas_remaining)
- [`mod@log`]: contract logging

Other useful modules and types:

- [`mod@types`]
  - [`ContractAddress`](ContractAddress)
  - [`StorageKey`](StorageKey)
  - [`PublicKey`](PublicKey)

# Interfaces And Specs

`#[contract]` on a *trait* defines a reusable interface; `#[contract]` on an
impl of that trait exports an implementation. Callers depend on the interface
crate and get the same generated client either way. That is how the standard
interfaces in `standard-interfaces/` (`groundhog-erc20`,
`groundhog-erc20-metadata`, `groundhog-signed-op`) are consumed.

Every `#[contract]` trait also carries an [`InterfaceId`] as
`Trait::INTERFACE_ID`, derived from its name and major version, and every
`#[contract]` impl block leaves a record of its interface's id in the
contract's [interface table](mod@interface_table). `groundhog-erc165` is the
query surface: `impl_erc165!()` adds a `supports_interface` entrypoint
answered by `implements!(id)` from the impls that actually linked. A
contract without it carries no table in its deployed image.

Alongside the entrypoints, the macros emit a machine-readable **contract
spec**: the interface's methods, their argument and return types, and the
exceptions it declares. Types reach the spec through
[`#[contract_type]`](macro@crate::contract_type) or
[`ContractTypeSpec`](derive@crate::ContractTypeSpec).

# Testing

Three testing lanes, all behind feature `dev`:

- [`client_test`]: simplest path for exercising a generated client against a
  mock registration
- [`TestEnvironment`](crate::TestEnvironment): direct control over environment,
  blocks, deployment, and multi-contract scenarios
- [`test_rv32()`]: run a built rv32 contract artifact in the test harness

Typical native client test, with the contract crate calling
`sdk::declare_mock_contract!();` in its `lib.rs` to generate
`register_mock_contract`:

```rust,ignore
use my_contract::MyContractClient;

sdk::client_test(
    |env| my_contract::register_mock_contract(env),
    |client: &MyContractClient| {
        // Call methods on `client`
    },
);
```

Direct environment control:

```rust,ignore
use sdk::{ContractAddress, Named, StorageKey, TestEnvironment};

let env = TestEnvironment::no_cost();
let addr = ContractAddress::named("my_test");

env.transact(|tx| {
    tx.call(&addr).run(|| {
        let key = StorageKey::default();
        sdk::bytestring_set(&key, b"hello");
    })
})
.assert_success();
```

Use the full `env.transact` form when a test needs multiple transactions,
asserts on aborts or faults, or decodes typed results.

The chain settings an environment runs under -- the storage cost model, the
network hash, the genesis root, and the network config parameters -- come
from [`TestEnvironment::builder()`](crate::TestEnvironment::builder) when a
test wants something other than the defaults. It is entirely optional; the
constructors above are unaffected by it.

```rust,ignore
// A contract's behavior at the chain's live-hasher cap, without needing a
// chain configured that way. Every unset parameter keeps its stock value.
let env = TestEnvironment::builder()
    .no_cost()
    .with_max_sha256_buffers(2)
    .build();
```

Advanced helpers are available when you need lower-level control:

- [`contract_test!`]
- [`into_root_call_result()`]
- [`load_rv32_contract()`]
- [`test_rv32()`]
