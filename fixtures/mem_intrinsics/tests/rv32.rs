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

static NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("MEM_INTRINSICS_NONCE"));

/// Deploys the artifact and runs `test` against its client.
fn with_client(
    test: impl FnOnce(&MemIntrinsicsClient) + std::panic::UnwindSafe,
) {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let code = CodeHash::<MemIntrinsicsClient>::new(
        env.setup_create_contract(&contract),
    );
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            test(&code.deploy(&NONCE).client());
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
            // a word and a 32-byte block, and the disjoint case.
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

/// Gas for one more 32-byte block, measured between two lengths that are
/// both whole blocks. Each method's other work does not depend on the
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
// or an addition 1. A block of eight words then costs 36 to copy and 11 to
// fill. The bounds leave room for a different instruction selection, and
// fail if a loop goes back to one word per iteration: that costs at least
// 64 per block to copy and 32 to fill.

#[test]
fn rv32_copy_is_unrolled() {
    with_client(|c| {
        let gas = gas_per_block(|len| {
            c.copy(len, 0, 0);
        });
        assert!(gas <= 44, "memcpy: {gas} gas per 32 bytes");
    });
}

#[test]
fn rv32_forward_shift_is_unrolled() {
    with_client(|c| {
        // The destination is below the source, so the move runs forward.
        let gas = gas_per_block(|len| {
            c.shift(len, 40, 8);
        });
        assert!(gas <= 44, "memmove: {gas} gas per 32 bytes");
    });
}

#[test]
fn rv32_disjoint_shift_above_source_is_unrolled() {
    with_client(|c| {
        // The destination is past the source's end, so nothing overlaps
        // and the move can still run forward.
        let gas = gas_per_block(|len| {
            c.shift(len, 8, 260);
        });
        assert!(gas <= 44, "memmove: {gas} gas per 32 bytes");
    });
}

#[test]
fn rv32_fill_is_unrolled() {
    with_client(|c| {
        let gas = gas_per_block(|len| {
            c.fill(len, 0, 0x5A);
        });
        assert!(gas <= 16, "memset: {gas} gas per 32 bytes");
    });
}
