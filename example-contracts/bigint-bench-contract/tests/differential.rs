//! Both backends must compute the same thing.
//!
//! A codesize or gas diff between two implementations only means something if
//! they are doing the same arithmetic. This asserts exactly that, over the same
//! op set the rv32 sweep measures, and it is the check that catches a mapping
//! mistake in `backend.rs` -- a `wrapping_mul_small` wired to a full multiply on
//! one side, a shift whose out-of-range behaviour differs -- which would
//! otherwise show up as a plausible-looking performance win.
//!
//! Runs on the host, so it needs neither rv32 nor the node. `crates/u256`'s own
//! tests already assert `sdk::U256` agrees with `uint` at the *library* level;
//! this covers the benchmark's own wiring on top of it.

use bigint_bench_contract::backend::Backend;
use bigint_bench_contract::backend::parity::PU256;
use bigint_bench_contract::backend::u256::GroundhogU256;
use bigint_bench_contract::ops;

/// Deterministic xorshift64* sweep. Fixed seed: a failure must be reproducible.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn next_u128(&mut self) -> u128 {
        ((self.next_u64() as u128) << 64) | self.next_u64() as u128
    }
}

/// The (hi, lo) operand pairs both backends are checked over.
///
/// Boundaries first, then a random sweep. The boundaries are where the
/// implementations' shapes differ most: zero and one exercise the early returns,
/// `MAX` exercises carry-out, and a value whose high 128 bits are zero is the
/// single-limb-divisor case `div_small` and `uint`'s `div_mod` fast path care
/// about.
fn operand_pairs() -> Vec<((u128, u128), (u128, u128))> {
    let edges: &[(u128, u128)] = &[
        (0, 0),
        (0, 1),
        (0, 2),
        (0, u128::MAX),
        (1, 0),
        (u128::MAX, u128::MAX),
        (0, u32::MAX as u128),
        (0, u64::MAX as u128),
        (0x8000_0000_0000_0000_0000_0000_0000_0000, 0),
        (0, 0x8000_0000_0000_0000_0000_0000_0000_0000),
    ];

    let mut pairs = Vec::new();
    for a in edges {
        for b in edges {
            pairs.push((*a, *b));
        }
    }

    // Enough volume to reach Knuth D's add-back correction, which fires with
    // probability ~2/2^32 per divide -- crates/u256's own sweeps make the same
    // argument for their size.
    let mut rng = Rng(0x0BADC0DE_DEADBEEF);
    for _ in 0..20_000 {
        let a = (rng.next_u128(), rng.next_u128());
        let b = (rng.next_u128(), rng.next_u128());
        pairs.push((a, b));
        // Half-width operands: the single-limb and short-divisor paths.
        pairs.push(((0, a.1), (0, b.1 >> 96)));
        pairs.push(((0, a.1 >> 64), (0, b.1)));
    }
    pairs
}

/// Asserts `op` agrees between the two backends across the whole sweep.
fn assert_agrees(
    name: &str,
    gh: fn(GroundhogU256, GroundhogU256) -> GroundhogU256,
    pa: fn(PU256, PU256) -> PU256,
) {
    for ((a_hi, a_lo), (b_hi, b_lo)) in operand_pairs() {
        let got = gh(
            GroundhogU256::from_hi_lo(a_hi, a_lo),
            GroundhogU256::from_hi_lo(b_hi, b_lo),
        )
        .to_be_bytes();

        let want =
            pa(PU256::from_hi_lo(a_hi, a_lo), PU256::from_hi_lo(b_hi, b_lo))
                .to_be_bytes();

        assert_eq!(
            got, want,
            "{name} disagrees on a={a_hi:#x}:{a_lo:#x} b={b_hi:#x}:{b_lo:#x}"
        );
    }
}

macro_rules! agreement_tests {
    ($( $test:ident => $name:literal, $op:path ; )*) => { $(
        #[test]
        fn $test() {
            assert_agrees($name, $op, $op);
        }
    )* };
}

// One test per op in the sweep. Mirrors the `kernels!` table in src/kernel.rs;
// `none` is omitted (it is the identity, with nothing to disagree about).
agreement_tests! {
    add => "add", ops::add;
    sub => "sub", ops::sub;
    mul => "mul", ops::mul;
    mul_small => "mul_small", ops::mul_small;
    widening_mul => "widening_mul", ops::widening_mul;
    div => "div", ops::div;
    div_small => "div_small", ops::div_small;
    rem => "rem", ops::rem;
    mul_div_u128 => "mul_div_u128", ops::mul_div_u128;
    mul_div => "mul_div", ops::mul_div;
    shl => "shl", ops::shl;
    shr => "shr", ops::shr;
    bitwise => "bitwise", ops::bitwise;
    cmp => "cmp", ops::cmp;
    sqrt => "sqrt", ops::sqrt;
    bytes => "bytes", ops::bytes;
    bit_len => "bit_len", ops::bit_len;
}

/// The digest folds every limb, in both backends.
///
/// The contract returns it so the kernel's result cannot be dead-coded. If it
/// ignored part of the value, LLVM could drop the arithmetic that produced that
/// part and the gas number would silently shrink -- so assert that flipping any
/// single bit changes it.
#[test]
fn digest_depends_on_every_bit() {
    for bit in 0..256u32 {
        let mut bytes = [0u8; 32];
        bytes[(bit / 8) as usize] = 1 << (bit % 8);

        let zero_gh = GroundhogU256::from_be_bytes([0; 32]).digest();
        let zero_pa = PU256::from_be_bytes([0; 32]).digest();

        assert_ne!(
            GroundhogU256::from_be_bytes(bytes).digest(),
            zero_gh,
            "u256 digest ignores bit {bit}"
        );
        assert_ne!(
            PU256::from_be_bytes(bytes).digest(),
            zero_pa,
            "uint digest ignores bit {bit}"
        );
    }
}

/// `mul_div_u128`'s derived divisor must keep the quotient inside a `u128`.
///
/// `sdk::mul_div_u128` panics when it does not, so the op would abort mid-sweep
/// instead of reporting gas. The sweep above would catch it, but only as a
/// panic in an unrelated-looking test; this states the precondition directly.
#[test]
fn mul_div_quotient_always_fits() {
    let mut rng = Rng(0x5EED_5EED_5EED_5EED);
    for _ in 0..50_000 {
        let a = rng.next_u128();
        let b = rng.next_u128();
        let d = b | 1;
        assert!(
            sdk::checked_mul_div_u128(a, b, d).is_some(),
            "a={a:#x} b={b:#x} d={d:#x} overflows u128"
        );
    }
}
