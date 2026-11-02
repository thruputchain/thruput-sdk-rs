//! The measured loop, and the `op-*` feature that picks what it runs.

use crate::backend::Backend;
use crate::ops;
use core::hint::black_box;

/// Number of `op-*` features enabled.
const OPS_SELECTED: usize = cfg!(feature = "op-none") as usize
    + cfg!(feature = "op-add") as usize
    + cfg!(feature = "op-sub") as usize
    + cfg!(feature = "op-mul") as usize
    + cfg!(feature = "op-mul-small") as usize
    + cfg!(feature = "op-widening-mul") as usize
    + cfg!(feature = "op-div") as usize
    + cfg!(feature = "op-div-small") as usize
    + cfg!(feature = "op-rem") as usize
    + cfg!(feature = "op-mul-div-u128") as usize
    + cfg!(feature = "op-mul-div") as usize
    + cfg!(feature = "op-mul-div-and-div") as usize
    + cfg!(feature = "op-signed-mul") as usize
    + cfg!(feature = "op-signed-div") as usize
    + cfg!(feature = "op-shl") as usize
    + cfg!(feature = "op-shr") as usize
    + cfg!(feature = "op-bitwise") as usize
    + cfg!(feature = "op-cmp") as usize
    + cfg!(feature = "op-sqrt") as usize
    + cfg!(feature = "op-bytes") as usize
    + cfg!(feature = "op-bit-len") as usize;

// A build carrying two kernels would link both implementations' code and
// silently report the sum; a build carrying none would not compile `run`. Both
// are worth failing loudly for, since the symptom is a plausible-looking
// number rather than an error.
const _: () = assert!(
    OPS_SELECTED == 1,
    "bigint-bench-contract: enable exactly one op-* feature \
     (scripts/bigint-bench.sh passes one per build)"
);

/// Applies `f` to the operands `iters` times.
///
/// `black_box` on both inputs and the result is what makes this a measurement
/// rather than a constant fold: without it LLVM hoists the whole call out of
/// the loop, and with `iters` known only at run time it would still collapse
/// the body to a single evaluation.
///
/// The `op-none` build runs this same loop with an identity `f`, so subtracting
/// its gas removes the loop counter, the two operand round trips through
/// memory, the call frame, and the entrypoint dispatch -- leaving the
/// operation.
#[inline(never)]
pub fn drive<B, F>(iters: u32, a: B, b: B, f: F) -> B
where
    B: Backend,
    F: Fn(B, B) -> B,
{
    let mut last = a;
    let mut i = 0u32;
    while i < iters {
        last = black_box(f(black_box(a), black_box(b)));
        i = i.wrapping_add(1);
    }
    last
}

/// Generates the one `run` the build's `op-*` feature selects.
///
/// Keep this table in sync with the `op-*` feature list in Cargo.toml and the
/// `OPS` list in scripts/bigint-bench.sh -- the three together are the op menu.
macro_rules! kernels {
    ($( $feature:literal => $name:literal, $op:path ; )*) => { $(
        /// The operation this build measures.
        #[cfg(feature = $feature)]
        pub const OP_NAME: &str = $name;

        /// Runs the selected operation `iters` times.
        #[cfg(feature = $feature)]
        pub fn run<B: Backend>(iters: u32, a: B, b: B) -> B {
            drive(iters, a, b, $op)
        }
    )* };
}

kernels! {
    "op-none"         => "none",           ops::none;
    "op-add"          => "add",            ops::add;
    "op-sub"          => "sub",            ops::sub;
    "op-mul"          => "mul",            ops::mul;
    "op-mul-small"    => "mul_small",      ops::mul_small;
    "op-widening-mul" => "widening_mul",   ops::widening_mul;
    "op-div"          => "div",            ops::div;
    "op-div-small"    => "div_small",      ops::div_small;
    "op-rem"          => "rem",            ops::rem;
    "op-mul-div-u128" => "mul_div_u128",   ops::mul_div_u128;
    "op-mul-div"      => "mul_div",        ops::mul_div;
    "op-mul-div-and-div" => "mul_div_and_div", ops::mul_div_and_div;
    "op-signed-mul"   => "signed_mul",     ops::signed_mul;
    "op-signed-div"   => "signed_div",     ops::signed_div;
    "op-shl"          => "shl",            ops::shl;
    "op-shr"          => "shr",            ops::shr;
    "op-bitwise"      => "bitwise",        ops::bitwise;
    "op-cmp"          => "cmp",            ops::cmp;
    "op-sqrt"         => "sqrt",           ops::sqrt;
    "op-bytes"        => "bytes",          ops::bytes;
    "op-bit-len"      => "bit_len",        ops::bit_len;
}
