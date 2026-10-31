//! Measures rv32 gas for a fixed workload per example contract, for the
//! `sdk_perf_gas` metric.
//!
//! Each row deploys the lowered artifacts from `target/thruput` into a fresh
//! [`TestEnvironment`] and reads `stats::gas_remaining()` either side of the
//! workload, so deployment is excluded:
//!
//! - `erc20`, `erc20_low_level`, `erc20_raw_storage`: the `thruput-erc20-test`
//!   conformance suite, driven from the host.
//! - `hasher`: its `run` entrypoint.
//! - `ref_args`, `u256_roundtrip`, `versioned_impl`: the matching `*_caller`
//!   contract's `run`, which calls the contract from a guest -- the same
//!   checks the bottle tests ran before #558 moved them into those callers.
//!
//! Output, one line per contract (the format the metric has always had):
//!
//! ```text
//! <contract>\tgas_consumed\t<gas>
//! ```

use std::path::{Path, PathBuf};

use hasher::HasherClient;
use ref_args_caller::RefArgsCallerClient;
use thruput_sdk::*;
use u256_roundtrip_caller::U256RoundtripCallerClient;
use versioned_impl_caller::VersionedImplCallerClient;

static CALLEE_NONCE: SHA256Hash = sha256_static!("SDK_PERF_GAS_CALLEE_NONCE");
static CALLER_NONCE: SHA256Hash = sha256_static!("SDK_PERF_GAS_CALLER_NONCE");

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/thruput".to_string()),
    );

    for name in ["erc20", "erc20_low_level", "erc20_raw_storage"] {
        report(name, measure_erc20(&artifact(&dir, name)));
    }
    report("hasher", measure_hasher(&artifact(&dir, "hasher")));
    report(
        "ref_args",
        measure_caller(&dir, "ref_args", |caller, callee| {
            RefArgsCallerClient::new(caller).run(callee)
        }),
    );
    report(
        "u256_roundtrip",
        measure_caller(&dir, "u256_roundtrip", |caller, callee| {
            U256RoundtripCallerClient::new(caller).run(callee)
        }),
    );
    report(
        "versioned_impl",
        measure_caller(&dir, "versioned_impl", |caller, callee| {
            VersionedImplCallerClient::new(caller).run(callee)
        }),
    );
}

fn report(name: &str, gas: u64) {
    println!("{name}\tgas_consumed\t{gas}");
}

/// The lowered artifact for `name`; a missing one fails loudly rather than
/// dropping a row from the append-only history.
fn artifact(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(format!("{name}.rv32a"));
    if !path.is_file() {
        panic!(
            "missing rv32 artifact {} (build with scripts/dev.sh build-examples-rv32)",
            path.display()
        );
    }
    path
}

/// Gas consumed by `f`, which runs inside a contract frame.
fn gas_of(f: impl FnOnce()) -> u64 {
    let before = stats::gas_remaining();
    f();
    let after = stats::gas_remaining();
    before
        .checked_sub(after)
        .unwrap_or_else(|| panic!("gas_remaining went up across the workload"))
}

fn measure_erc20(path: &Path) -> u64 {
    let env = TestEnvironment::no_cost();
    let hash = env.setup_create_contract(&load_rv32_contract(path));

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("sdk_perf_gas"))
            .run(move || {
                let addr = thruput_erc20_test::erc20_test_setup(&hash);
                gas_of(|| {
                    let _ = thruput_erc20_test::erc20_test(&addr);
                })
            })
    });
    result.assert_success();
    result.assert_return_as()
}

fn measure_hasher(path: &Path) -> u64 {
    let env = TestEnvironment::no_cost();
    let hash = env.setup_create_contract(&load_rv32_contract(path));

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("sdk_perf_gas"))
            .run(move || {
                let addr = deploy_contract(&CALLEE_NONCE, &hash);
                gas_of(|| HasherClient::new(&addr).run())
            })
    });
    result.assert_success();
    result.assert_return_as()
}

/// Deploys `<callee>` and `<callee>_caller` and measures the caller's `run`,
/// which calls the callee guest-to-guest.
fn measure_caller(
    dir: &Path,
    callee: &str,
    run: fn(&ContractAddress, &ContractAddress),
) -> u64 {
    let callee_contract = load_rv32_contract(artifact(dir, callee));
    let caller_contract =
        load_rv32_contract(artifact(dir, &format!("{callee}_caller")));

    let env = TestEnvironment::no_cost();
    let callee_hash = env.setup_create_contract(&callee_contract);
    let caller_hash = env.setup_create_contract(&caller_contract);

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("sdk_perf_gas"))
            .run(move || {
                let callee_addr = deploy_contract(&CALLEE_NONCE, &callee_hash);
                let caller_addr = deploy_contract(&CALLER_NONCE, &caller_hash);
                gas_of(|| run(&caller_addr, &callee_addr))
            })
    });
    result.assert_success();
    result.assert_return_as()
}
