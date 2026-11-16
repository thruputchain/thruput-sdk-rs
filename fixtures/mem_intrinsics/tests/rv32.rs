//! The rv32 artifact, and with it the runtime's `memcpy`, `memmove` and
//! `memset`: results against the model in `common`, and gas per byte.
//!
//! Requires the lowered `mem_intrinsics` artifact:
//! `cargo gnaw build mem_intrinsics` writes it.

mod common;
use common::lengths;
use mem_intrinsics::MemIntrinsicsClient;
use thruput_sdk::*;

// Test working directory is the example contract directory.
const RV32_PATH: &str = "../../target/thruput/mem_intrinsics.rv32a";

static NONCE: SHA256Hash = sha256_static!("MEM_INTRINSICS_NONCE");

/// Deploys the artifact and runs `test` against its client.
fn with_client(
    test: impl FnOnce(&MemIntrinsicsClient) + std::panic::UnwindSafe,
) {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let hash = env.setup_create_contract(&contract);
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&NONCE, &hash);
            test(&MemIntrinsicsClient::new(&addr));
        })
    })
    .assert_success();
}

#[test]
fn rv32_copy_matches_model() {
    with_client(|c| {
        for len in lengths() {
            // Source and destination at each alignment relative to a word.
            for (src, dst) in [(0, 0), (1, 0), (0, 3), (2, 1)] {
                assert_eq!(
                    c.copy(len, src, dst),
                    common::copy(len, src, dst),
                    "copy len={len} src={src} dst={dst}"
                );
            }
        }
    });
}

#[test]
fn rv32_shift_matches_model() {
    with_client(|c| {
        let src = 50u32;
        for len in lengths() {
            // Overlaps in both directions, at distances below, at and above
            // a word, and well past one.
            for distance in
                [-33i32, -32, -31, -5, -4, -3, -1, 0, 1, 3, 4, 5, 31, 32, 33]
            {
                let dst = src.checked_add_signed(distance).unwrap();
                assert_eq!(
                    c.shift(len, src, dst),
                    common::shift(len, src, dst),
                    "shift len={len} src={src} dst={dst}"
                );
            }
        }
    });
}

#[test]
fn rv32_fill_matches_model() {
    with_client(|c| {
        for len in lengths() {
            for (dst, byte) in [(0, 0), (1, 0xFF), (2, 0x5A), (3, 0x1FF)] {
                assert_eq!(
                    c.fill(len, dst, byte),
                    common::fill(len, dst, byte),
                    "fill len={len} dst={dst} byte={byte:#x}"
                );
            }
        }
    });
}

/// Gas for 32 more bytes, measured between two lengths that are both whole
/// words. Each method's other work does not depend on the
/// length, so the difference is the routine's loop alone. Both lengths
/// encode to the same number of calldata bytes, so decoding them costs the
/// same.
fn gas_per_block(call: impl Fn(u32)) -> u64 {
    const SHORT: u32 = 96;
    const LONG: u32 = 224;
    let gas = |len: u32| {
        let before = stats::gas_remaining();
        call(len);
        before - stats::gas_remaining()
    };
    (gas(LONG) - gas(SHORT)) / ((LONG - SHORT) / 32) as u64
}

// In the test executor a load costs 3 gas, a taken branch 2, and a store
// or an addition 1. A five-instruction copy loop then costs 64 per 32
// bytes, and a three-instruction fill loop 32. One more instruction per
// word, which is what these loops once carried, costs 8 more.

#[test]
fn rv32_copy_takes_five_instructions_per_word() {
    with_client(|c| {
        let gas = gas_per_block(|len| {
            c.copy(len, 0, 0);
        });
        assert!(gas <= 64, "memcpy: {gas} gas per 32 bytes");
    });
}

#[test]
fn rv32_forward_shift_takes_five_instructions_per_word() {
    with_client(|c| {
        // The destination is below the source, so the move runs forward.
        let gas = gas_per_block(|len| {
            c.shift(len, 40, 8);
        });
        assert!(gas <= 64, "memmove: {gas} gas per 32 bytes");
    });
}

#[test]
fn rv32_fill_takes_three_instructions_per_word() {
    with_client(|c| {
        let gas = gas_per_block(|len| {
            c.fill(len, 0, 0x5A);
        });
        assert!(gas <= 32, "memset: {gas} gas per 32 bytes");
    });
}
