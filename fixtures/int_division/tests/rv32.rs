//! The rv32 artifact, and with it the runtime's 64-bit and 128-bit
//! division: results against host arithmetic, and gas per division.
//!
//! Requires the lowered `int_division` artifact:
//! `cargo gnaw build int_division` writes it.

mod common;
use common::*;
use int_division::IntDivisionClient;
use thruput_sdk::*;

// Test working directory is the example contract directory.
const RV32_PATH: &str = "../../target/thruput/int_division.rv32a";

static NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("INT_DIVISION_NONCE"));

/// Deploys the artifact and runs `test` against its client.
fn with_client(
    test: impl FnOnce(&IntDivisionClient) + std::panic::UnwindSafe,
) {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let code = CodeHash::<IntDivisionClient>::new(
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
fn rv32_op64_matches_host_arithmetic() {
    with_client(|c| {
        for kind in KINDS {
            for a in OPERANDS_64 {
                for b in OPERANDS_64 {
                    // `None` is the signed quotient that overflows.
                    let Some(expected) = op64(kind, a, b) else {
                        continue;
                    };
                    assert_eq!(
                        c.op64(kind, a, b, 1),
                        expected,
                        "kind={kind} a={a:#x} b={b:#x}"
                    );
                }
            }
        }
    });
}

#[test]
fn rv32_op128_matches_host_arithmetic() {
    with_client(|c| {
        for kind in KINDS {
            for a in OPERANDS_128 {
                for b in OPERANDS_128 {
                    let Some(expected) = op128(kind, a, b) else {
                        continue;
                    };
                    assert_eq!(
                        c.op128(kind, a, b, 1),
                        expected,
                        "kind={kind} a={a:#x} b={b:#x}"
                    );
                }
            }
        }
    });
}

/// Gas for one more division, measured between two round counts. The
/// figure includes the loop around the division: a few instructions that
/// move the operands and the result through memory.
fn gas_per_division(call: impl Fn(u32)) -> u64 {
    const FEW: u32 = 100;
    const MANY: u32 = 300;
    let gas = |rounds: u32| {
        let before = stats::gas_remaining();
        call(rounds);
        before - stats::gas_remaining()
    };
    (gas(MANY) - gas(FEW)) / (MANY - FEW) as u64
}

// Each bound is between one and a half and two times what the division
// costs. A 128-bit division is the runtime's `U128MulDivRem`: about 225 gas
// unsigned, 245 signed with positive operands. The code these replaced cost
// several times the bound: compiler_builtins' 64-bit division takes about 670
// gas for the operands below, and LLVM's inline 128-bit division 6,400 to
// 10,000.

#[test]
fn rv32_u64_division_by_two_limbs_is_bounded() {
    with_client(|c| {
        let gas = gas_per_division(|rounds| {
            c.op64(
                UNSIGNED_QUOTIENT,
                0xFEDC_BA98_7654_3210,
                0x1_0000_0001,
                rounds,
            );
        });
        assert!(gas <= 330, "u64 division: {gas} gas");
    });
}

#[test]
fn rv32_u128_division_by_a_small_constant_is_bounded() {
    with_client(|c| {
        let gas = gas_per_division(|rounds| {
            c.op128(UNSIGNED_QUOTIENT, u128::MAX / 3, 10_000, rounds);
        });
        assert!(gas <= 400, "u128 division by 10,000: {gas} gas");
    });
}

#[test]
fn rv32_u128_division_by_two_limbs_is_bounded() {
    with_client(|c| {
        let gas = gas_per_division(|rounds| {
            c.op128(
                UNSIGNED_QUOTIENT,
                u128::MAX / 3,
                1_000_000_000_000_000_000,
                rounds,
            );
        });
        assert!(gas <= 400, "u128 division by 10^18: {gas} gas");
    });
}

#[test]
fn rv32_i128_division_of_positive_amounts_is_bounded() {
    with_client(|c| {
        let gas = gas_per_division(|rounds| {
            c.op128(
                SIGNED_QUOTIENT,
                u128::MAX / 3,
                1_000_000_000_000_000_000,
                rounds,
            );
        });
        assert!(gas <= 400, "i128 division by 10^18: {gas} gas");
    });
}

#[test]
fn rv32_zero_divisor_and_overflowing_quotient_still_trap() {
    with_client(|c| {
        // The language checks both before it divides, whatever the
        // profile's `overflow-checks` says, so the routines never see
        // these operands. The contract profile turns `overflow-checks` off.
        for kind in KINDS {
            assert!(c.try_op64(kind, 7, 0, 1).is_err(), "u64 kind={kind}");
            assert!(c.try_op128(kind, 7, 0, 1).is_err(), "u128 kind={kind}");
        }
        for kind in [SIGNED_QUOTIENT, SIGNED_REMAINDER] {
            assert!(
                c.try_op64(kind, 1 << 63, u64::MAX, 1).is_err(),
                "i64 kind={kind}"
            );
            assert!(
                c.try_op128(kind, 1 << 127, u128::MAX, 1).is_err(),
                "i128 kind={kind}"
            );
        }
        // The contract still answers after each failed call.
        assert_eq!(c.op64(UNSIGNED_QUOTIENT, 7, 2, 1), 3);
    });
}
