# Getting Started with the Groundhog Rust SDK

This guide introduces the Groundhog SDK for writing smart contracts in Rust.

---

## What is Groundhog?

Groundhog is an L1 blockchain designed for 1M+ TPS with irrevocable finality and zero gas fees.

### The Core Innovation: Commutative Semantics

Unlike every other blockchain where transaction order matters, Groundhog transactions produce **storage deltas** that merge deterministically regardless of execution order. All transactions in a block execute against the **same snapshot** - they can't see each other's writes, enabling unlimited parallelization.

This has some neat upsides:

- 1000 transfers to the same account execute **in parallel** and merge correctly
- Complex, compute heavy smart contracts don't get in the way of simpler operations
- As a result, no transaction failures from congestion
- No MEV extraction through ordering manipulation

More glorious docs here: https://groundhog.dev/

---

## Repository Structure

```
sdk_rs/
├── src/                    # Core SDK
│   ├── lib.rs              # Entry point - exports everything
│   ├── storage/            # High-level storage (Map, Vector, NNInt)
│   ├── invoke.rs           # Cross-contract calls
│   ├── event.rs            # Event logging
│   └── contract_op.rs      # Contract entry/exit
├── macros/                 # Procedural macros (#[contract], #[storage])
├── gen/                    # Code generation used by macros
├── examples/               # Example contracts (start here!)
│   ├── counter/            # Simple counter (best starting point)
│   ├── erc20/              # Full ERC20 token
│   ├── crane_rewards/      # DePIN example with low-level storage
│   └── ...                 # Other examples (caller, hasher, etc.)
├── standard-interfaces/    # Trait definitions (ERC20, etc.)
├── native_exec/            # Mock execution for testing
└── tests/                  # SDK unit tests
```

---

## Key Storage Types

Groundhog provides **commutative primitives** - these are fundamentally different from EVM storage.

### Base Primitives

| Type | Purpose | Commutativity |
|------|---------|---------------|
| `NNInt` | Non-negative integer (balances, counters) | Additive ops always commute |
| `Bytestring` | Raw bytes | Commutes only if same value written |

These are the two fundamental storage types. `NNInt` is the workhorse - use it for any numeric value that should never go negative (balances, counters, totals).

### Composite Types

| Type | Purpose | Commutativity |
|------|---------|---------------|
| `Map<K, V>` | Persistent key-value store | Operations on different keys commute |
| `Vector<T>` | Dynamic array (**experimental**) | Append operations commute |

`Map` wraps the base primitives - each key is independent, so `map[A] += 5` and `map[B] -= 3` always commute.

`Vector` is experimental. Only append operations commute; the data type's semantics may evolve.

### Why This Matters

Traditional blockchain (last-write-wins):
```
Initial balance: 100
Tx A: set balance = 150  (user deposits 50)
Tx B: set balance = 80   (user withdraws 20)
Result: 80 or 150 depending on order (one tx lost!)
```

Groundhog (delta-based):
```
Initial balance: 100
Tx A: balance += 50
Tx B: balance -= 20
Result: 130 (always, regardless of order)
```

---

## Writing Your First Contract

### Basic Structure

Here's a minimal counter contract (see `examples/counter/` for the full version):

```rust
use sdk::storage::NNInt;
use sdk::*;
use thiserror::Error;

// 1. Define error type
#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    #[error("Counter would go negative")]
    WouldGoNegative = 1,
    #[error("System error")]
    SysErr = 2,
}

impl From<SysErr> for Error {
    fn from(_: SysErr) -> Self { Error::SysErr }
}

impl From<sysresult::Error> for Error {
    fn from(err: sysresult::Error) -> Self {
        match err {
            sysresult::Error::NonNegativeIntUnderflow => Error::WouldGoNegative,
            _ => Error::SysErr,
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

// 2. Define state with #[storage]
#[derive(Debug)]
#[storage]
pub struct Counter {
    value: NNInt,
}

// 3. Implement with #[contract]
#[contract(exports = "guest_exports", err_type = Error, client = CounterClient)]
impl Counter {
    #[init]
    pub fn initialize(&mut self, initial_value: u64) -> Result<()> {
        self.value.checked_inc(initial_value as i128)?;
        Ok(())
    }

    pub fn increment(&mut self, amount: u64) -> Result<()> {
        self.value.checked_inc(amount as i128)?;
        Ok(())
    }

    pub fn decrement(&mut self, amount: u64) -> Result<()> {
        self.value.checked_inc(-(amount as i128))?;
        Ok(())
    }

    pub fn get(&self) -> Result<u64> {
        Ok(self.value.try_get()?.unwrap_or(0) as u64)
    }
}
```

### Key Patterns

1. **`#[storage]`** - Marks your contract state struct. Fields are automatically persisted.

2. **`#[contract]`** - Generates WASM exports and client code. Parameters:
   - `exports` - Module name for exported functions
   - `err_type` - Your error enum
   - `client` - (optional) Generate a client struct for cross-contract calls

3. **`#[init]`** - Initialization method, called only on first invocation.

4. **`&mut self` vs `&self`** - Mutating methods persist state; read-only methods don't.

5. **`checked_inc(delta)`** - The core commutative operation. Use this instead of assignment.

### Error Handling

Your error type needs `From` impls for `SysErr` and `sysresult::Error` (shown in the example above). Add to `Cargo.toml`:

```toml
thiserror = { workspace = true }
xdrs = { workspace = true, features = ["derive"] }
```

> **Note:** Error handling is being simplified — see [What's Coming](#whats-coming) for details on the new panic-based approach that eliminates most of this boilerplate.

---

## Building and Testing

The SDK provides a `./scripts/make.sh` script for common operations:

### Build for WASM

```bash
# Build a specific example
./scripts/make.sh examples build erc20

# Or directly with cargo
cargo build --package erc20 --target wasm32-unknown-unknown --features guest
```

### Run Tests

Tests use the `native_exec` mock environment. On supported platforms (x86_64 Linux, aarch64 macOS), prebuilt test libraries are automatically downloaded — no C++ toolchain required.

```bash
# Test the SDK
./scripts/make.sh test

# Test all examples
./scripts/make.sh examples test

# Test a specific example
./scripts/make.sh examples test erc20
```

### Test Patterns

Tests use `TestEnvironment` to set up a mock execution environment. From `examples/counter/tests/test.rs`:

```rust
use sdk::contract_test::*;
use sdk::*;

#[test]
fn test_counter_basic() {
    let mut env = TestEnvironment::new();

    // Register the contract with the test environment
    let test_contract = counter::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || {
        // Deploy the contract
        let nonce = SHA256Hash::from([1; 32]);
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();

        // Create a client to interact with the contract
        let mut client = counter::CounterClient::new(&deployed_addr);

        client.initialize(10).unwrap();
        assert_eq!(client.get().unwrap(), 10);

        client.increment(5).unwrap();
        assert_eq!(client.get().unwrap(), 15);
    })
    .unwrap();
}
```

---

## Cross-Contract Calls

The `#[contract]` macro can generate client stubs:

```rust
#[contract(
    exports = "guest_exports",
    err_type = Error,
    client = MyTokenClient  // Generates client struct
)]
impl MyToken { ... }
```

Calling another contract:

```rust
use other_contract::OtherClient;

pub fn call_other(&self, addr: &ContractAddress) -> Result<u128, Error> {
    let client = OtherClient::new(addr);
    let result = client.some_method()?;
    Ok(result)
}
```

---

## Key Differences from Solidity/EVM

| Concept | EVM | Groundhog |
|---------|-----|-----------|
| Language | Solidity | Rust → WASM |
| State updates | Absolute writes | Commutative deltas |
| Parallelism | Sequential or optimistic | True parallel execution |
| Gas | Per-opcode fees | Free compute, storage costs only |
| Finality | Probabilistic/delayed | Irrevocable via SCP |
| `approve()` | Sets absolute value (race condition) | Uses delta adjustments |

---

## Design Guidelines

### Do

- Use `NNInt` for all balances and counters
- Use `checked_inc(delta)` instead of assignment
- Design with deltas in mind: `+50`, `-20` not `set 130`
- Keep storage keys independent when possible

### Avoid

- Absolute writes to frequently-updated values (forces serialization)
- Reading a value and writing it back (use deltas instead)
- Complex conditional logic based on current state (may not commute)

---

## Examples to Study

**Current examples** (actively maintained):

| Example | What It Shows |
|---------|---------------|
| `examples/counter/` | Simplest complete example — start here |
| `examples/erc20/` | Production patterns: events, errors, allowances, trait impl |
| `examples/crane_rewards/` | DePIN pattern: low-level storage, owner checks, real-world use case |

**Other examples** (may be out of date):

| Example | What It Shows |
|---------|---------------|
| `examples/caller/` | Cross-contract invocation patterns |
| `examples/hasher/` | Minimal stateless contract |

Start with `counter/`, then study `erc20/src/implementation.rs` for production patterns. The `crane_rewards/` example shows an alternative approach using low-level storage functions directly (useful when you need more control).

---

## What's Coming

The SDK is actively evolving. Here's what's in development:

### Simplified Error Handling (`dbg/panic` branch)

The current `Result<T, Error>` pattern requires boilerplate for error type definitions and `From` impls. The new approach:

- Methods return values directly instead of `Result<T, Error>`
- `checked_inc()` becomes `strict_inc()` — aborts on underflow instead of returning an error
- `try_get()` returns `Option<T>` directly
- Errors abort the transaction via panic rather than propagating through `Result`

This matches blockchain semantics better: if something fails, the transaction should abort anyway.

### Retrocausal Queues (RCQ)

A new storage primitive for **sequencing** in a commutative world. Low-level syscalls exist (`retrocausal_queue_insert`, `retrocausal_queue_consume`), with a high-level `storage::` wrapper coming.

RCQ is a single-consumer, multi-producer FIFO queue:
- **Insert** operations from multiple parallel transactions commute (all entries are added)
- **Consume** operations sequence entries in FIFO order
- The "retrocausal" property: a consumer can see entries inserted by transactions that execute "later" in the same block (since all transactions see the same snapshot)

Use cases: order books, job queues, any pattern requiring deterministic ordering despite parallel execution.

---

## Further Reading

- `doc/macro-contract.md` - Details on the `#[contract]` macro
- `doc/macro-storage.md` - Details on the `#[storage]` macro
- `src/storage/` - How the storage primitives work internally
- `tests/storage.rs` - More usage patterns

---

## Getting Help

- Check the examples for common patterns
- Read the macro documentation for attribute options
- Look at `standard-interfaces/` for trait definitions
