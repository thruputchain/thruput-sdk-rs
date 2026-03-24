# Ergonomic Multi-Trait Contract Test Setup

## Problem

When a contract implements multiple traits, users must add redundant `exports` and `test_setup` annotations to every `#[contract]` attribute to avoid module name collisions:

```rust
#[contract(throws = AdderError, storage = "false", exports = "adder_exports", test_setup = "adder_test_setup")]
impl Adder for MathContract { ... }

#[contract(throws = MultiplierError, storage = "false", exports = "multiplier_exports", test_setup = "multiplier_test_setup")]
impl Multiplier for MathContract { ... }
```

Test registration also requires knowing these generated module names and manually wiring them together:

```rust
let mut c = env.add_contract();
adder_test_setup::register_mock(&mut c);
multiplier_test_setup::register_mock(&mut c);
c.as_contract()
```

## Solution

Replace the generated `mod test_setup` with a trait-based approach and auto-derive the exports module name from the trait name.

### 1. `TestSetup` trait

Add to `native_exec/src/mock_contract.rs`:

```rust
pub trait TestSetup<Iface: ?Sized> {
    fn register_mock(contract: &mut MockContract);
}
```

Re-export from `native_exec/src/lib.rs` and from `sdk/src/dev/mod.rs` (alongside the existing `MockContract` re-export).

### 2. `MockContract::with_impl` builder method

```rust
impl MockContract {
    pub fn with_impl<C: TestSetup<T>, T: ?Sized>(&mut self) -> &mut Self {
        C::register_mock(self);
        self
    }
}
```

Returns `&mut Self` for chaining:
```rust
let contract = env.add_contract()
    .with_impl::<MathContract, dyn Adder>()
    .with_impl::<MathContract, dyn Multiplier>()
    .as_contract();
```

### 3. `TestEnvironmentHandle::register_mock` one-liner

```rust
impl TestEnvironmentHandle {
    pub fn register_mock<C: TestSetup<T>, T: ?Sized>(&self) -> Contract {
        let mut c = self.add_contract();
        c.with_impl::<C, T>();
        c.as_contract()
    }
}
```

This returns `Contract` (the XDR enum) — same as the old `test_setup::register`. The caller still calls `setup_create_contract` separately, matching the existing pattern.

### 4. Codegen: replace `mod test_setup` with `impl TestSetup`

For trait impls (`impl Trait for Type`), `gen/src/test_reg.rs` generates:

```rust
impl ::sdk::TestSetup<dyn #trait_path> for #self_ty {
    fn register_mock(contract: &mut ::sdk::MockContract) {
        contract.register_method(#entrypoint, #exports_mod::#internal_name);
        // ... one per method
    }
}
```

This code is only emitted when `cfg!(feature = "mock")` is enabled on the codegen crate — the same gating mechanism used for the current `mod test_setup` generation in `macros/src/contract.rs`.

For bare impls (no trait), keep generating the `mod test_setup` module as before — there is no trait to parameterize on.

The orphan rule is satisfied because `#self_ty` is always local to the crate doing the impl.

### 5. Codegen: auto-derive exports module name from trait name

In `gen/src/config.rs`, when computing the exports module name for a trait impl with no explicit `exports = "..."`:

- Extract the last segment of the trait path (e.g., `Adder` from `some_crate::Adder`) — reuse `path_last_segment_ident` from `contract.rs`
- Convert to snake_case (add `heck` dependency to the `gen` crate, or write a simple CamelCase-to-snake_case converter)
- Append `_exports` (e.g., `adder_exports`)

For bare impls, keep the default `exports`. Explicit `exports = "..."` always takes precedence.

Note: some existing explicit names may differ from auto-derived names (e.g., swap contract uses `swap_exports` but `ISwapV2Pair` would auto-derive `i_swap_v2_pair_exports`). Those contracts can keep their explicit `exports = "..."` if desired, or accept the new auto-derived name.

### 6. Remove now-unnecessary annotations

Remove `exports = "..."` and `test_setup = "..."` from all `#[contract]` attributes on trait impls across example-contracts, integration_test, and groundhog-examples. For contracts where the explicit `exports` name intentionally differs from the auto-derived name (e.g., swap), decide per-contract whether to keep the explicit name or adopt the auto-derived one.

### 7. `test_setup` config option on trait impls

For trait impls, if `test_setup = "..."` is provided, emit a compile error directing the user to use the `TestSetup` trait instead. This prevents stale annotations from silently doing nothing after migration.

## User-facing API (before and after)

### Contract declaration

```rust
// Before:
#[contract(throws = AdderError, storage = "false", exports = "adder_exports", test_setup = "adder_test_setup")]
impl Adder for MathContract { ... }

// After:
#[contract(throws = AdderError, storage = "false")]
impl Adder for MathContract { ... }
```

### Tests (multi-trait)

```rust
// Before:
let mut c = env.add_contract();
adder_test_setup::register_mock(&mut c);
multiplier_test_setup::register_mock(&mut c);
c.as_contract()

// After:
let contract = env.add_contract()
    .with_impl::<MathContract, dyn Adder>()
    .with_impl::<MathContract, dyn Multiplier>()
    .as_contract();
```

### Tests (single-trait)

```rust
// Before:
let test_contract = counter::test_setup::register(&env);

// After:
let test_contract = env.register_mock::<Counter, dyn CounterTrait>();
```

### `client_test` migration

`client_test` takes `R: Fn(&TestEnvironmentHandle) -> Contract`. A closure wrapping `register_mock` works directly:

```rust
// Before:
sdk::client_test(test_setup::register, |c: &mut CounterClient| { ... });

// After:
sdk::client_test(
    |env| env.register_mock::<Counter, dyn CounterTrait>(),
    |c: &mut CounterClient| { ... },
);
```

No changes to `client_test` itself are needed.

### `pub use test_setup::*` re-exports

Crates that re-export `test_setup` (e.g., `hasher` does `pub use implementation::test_setup::*`) should replace with a manual wrapper or remove the re-export, since the `TestSetup` trait impl is automatically available to downstream crates that depend on the contract crate.

## Scope

### Files to modify (SDK)

- `native_exec/src/mock_contract.rs` — add `TestSetup` trait, `MockContract::with_impl`
- `native_exec/src/test_environment.rs` — add `TestEnvironmentHandle::register_mock`
- `native_exec/src/lib.rs` — re-export `TestSetup`
- `src/dev/mod.rs` — re-export `TestSetup` from native_exec (alongside `MockContract`)
- `gen/src/test_reg.rs` — generate `impl TestSetup` instead of `mod test_setup` for trait impls
- `gen/src/config.rs` — auto-derive exports module name from trait name; error on `test_setup` for trait impls
- `gen/src/exports.rs` — pass trait name through to config
- `gen/Cargo.toml` — add `heck` dependency (if not writing manual snake_case conversion)
- `macros/src/contract.rs` — pass trait name to codegen functions

### Files to migrate

- All test files in `example-contracts/`, `integration_test/`, and `../groundhog-examples/` that reference `test_setup::register` or `test_setup::register_mock`
- All `#[contract]` attributes with now-unnecessary `exports` / `test_setup` parameters on trait impls
- `example-contracts/hasher/src/lib.rs` — remove `pub use implementation::test_setup::*`

## Edge cases

- **Bare impls** (no trait): unchanged — keep generating `mod test_setup` and `mod exports` with current defaults
- **Explicit `exports = "..."`**: still honored, takes precedence over auto-derived name
- **`test_setup = "..."` on trait impls**: compile error after this change
- **Orphan rule**: always satisfied because `#self_ty` is local to the implementing crate
