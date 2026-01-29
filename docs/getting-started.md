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
│   ├── erc20/              # Full ERC20 token
│   ├── counter/            # Simple counter (good first example)
│   ├── caller/             # Cross-contract calls
│   ├── hasher/             # Minimal stateless contract
│   └── ...                 # More examples (swap, aborter, etc.)
├── standard-interfaces/    # Trait definitions (ERC20, etc.)
├── native_exec/            # Mock execution for testing
└── tests/                  # SDK unit tests
```

---

## Key Storage Types

Groundhog provides **commutative primitives** - these are fundamentally different from EVM storage:

| Type | Purpose | Commutativity |
|------|---------|---------------|
| `NNInt` | Non-negative integer (balances) | Pure additive ops always commute |
| `Map<K, V>` | Persistent key-value store | Keys commute independently |
| `Vector<T>` | Dynamic array | Append operations commute |
| `Bytestring` | Raw bytes | Commutes only if same value written |

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

```rust
use sdk::prelude::*;

// 1. Define state with #[storage]
#[derive(Debug)]
#[sdk::storage]
pub struct MyToken {
    balances: Map<ContractAddress, NNInt>,
    total_supply: NNInt,
}

// 2. Define error type
#[derive(Error, Debug, Clone, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    #[error("Insufficient balance")]
    InsufficientBalance,
}

// 3. Implement with #[contract]
#[contract(exports = "guest_exports", err_type = Error)]
impl MyToken {
    #[init]
    pub fn initialize(&mut self, initial_supply: u128) -> Result<(), Error> {
        let caller = get_caller()?;
        self.balances.get(&caller)?.checked_inc(initial_supply as i128)?;
        self.total_supply.checked_inc(initial_supply as i128)?;
        Ok(())
    }

    pub fn transfer(&mut self, to: &ContractAddress, amount: u128) -> Result<(), Error> {
        let from = get_caller()?;
        let diff = amount as i128;

        // Pure additive - these ALWAYS commute!
        self.balances.get(&from)?.checked_inc(-diff)?;
        self.balances.get(to)?.checked_inc(diff)?;

        Ok(())
    }

    pub fn balance_of(&self, addr: &ContractAddress) -> Result<u128, Error> {
        Ok(self.balances.get(addr)?.try_get()?.unwrap_or(0) as u128)
    }
}
```

### Key Patterns

1. **`#[storage]`** - Marks your contract state struct. Fields are automatically persisted.

2. **`#[contract]`** - Generates WASM exports and client code. Parameters:
   - `exports` - Module name for exported functions
   - `err_type` - Your error enum for `Err(...)` aborts
   - `client` - (optional) Generate a client struct for cross-contract calls

3. **`#[init]`** - Initialization method, called only on first invocation.

4. **`&mut self` vs `&self`** - Mutating methods persist state; read-only methods don't.

5. **`checked_inc(delta)`** - The core commutative operation. Use this instead of assignment.

### Error Handling

Custom errors require some boilerplate. Your error type needs:

```rust
use sdk::*;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum Error {
    #[error("Would go negative")]
    WouldGoNegative = 1,
    #[error("System error")]
    SysErr = 2,
}

// Required: convert low-level syscall errors
impl From<SysErr> for Error {
    fn from(_: SysErr) -> Self {
        Error::SysErr
    }
}

// Required: convert SDK errors (e.g., from NNInt operations)
impl From<sysresult::Error> for Error {
    fn from(err: sysresult::Error) -> Self {
        match err {
            sysresult::Error::NonNegativeIntUnderflow => Error::WouldGoNegative,
            _ => Error::SysErr,
        }
    }
}
```

And add to `Cargo.toml`:
```toml
xdrs = { workspace = true, features = ["derive"] }
```

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

Tests use `TestEnvironment` to set up a mock execution environment:

```rust
use sdk::contract_test::*;
use sdk::*;

#[test]
fn test_my_contract() {
    // Set up mock environment
    let mut env = TestEnvironment::new();

    // Register your contract
    let test_contract = my_contract::test_setup::register(&mut env);
    let contract_hash = env.setup_create_contract(&test_contract);

    // Run test inside mock contract context
    contract_test_invoke(env, move || {
        // Deploy contract
        let nonce = SHA256Hash::from([1; 32]);
        let deployed_addr = deploy_contract(&nonce, &contract_hash).unwrap();

        // Create client and interact
        let mut client = my_contract::MyContractClient::new(&deployed_addr);
        client.initialize(100).unwrap();

        // Assertions
        let value = client.get_value().unwrap();
        assert_eq!(value, 100);
    })
    .unwrap();
}
```

See `examples/counter/tests/test.rs` for a complete working example.

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

| Example | What It Shows |
|---------|---------------|
| `examples/counter/` | Simple counter — best starting point for learning the patterns |
| `examples/erc20/` | Complete token with events, errors, allowances, trait impl |
| `examples/caller/` | Cross-contract invocation patterns |
| `examples/hasher/` | Minimal stateless contract |
| `examples/aborter/` | Error handling and abort patterns |

Start with `examples/counter/` for the simplest complete example, then study `examples/erc20/src/implementation.rs` for production patterns.

*Note: Some examples (swap, etc.) may be disabled during development — check for a `disabled` file in the example directory.*

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
