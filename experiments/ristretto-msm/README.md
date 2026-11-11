# Ristretto255 MSM contract check

Checks the experimental host API through native and RV32 contract calls:
arithmetic, malformed inputs, the term limit and insufficient gas.

Use Thruput CLI 0.44.0 and
[hogd `86927c6c`](https://github.com/thruputchain/hogd/commit/86927c6c7915618bc0f80474061c80e16cee3f0f)
built with `HOGD_EXPERIMENTAL_RISTRETTO_MSM=ON`. Set `HOGD_FFI_LIB_DIR` to a directory
containing that build's `libtest_ffi` shared library and `bindings.rs`; see
[native executor setup](../../crates/native-exec/README.md).
The manifest pins the matching protocol bindings.

After building hogd's `test_ffi` target in `build-msm`, generate its bindings
from the hogd checkout:

```sh
hogd/test_ffi/run_bindgen.sh build-msm/hogd/test_ffi/bindings.rs
export HOGD_FFI_LIB_DIR="$PWD/build-msm/hogd/test_ffi"
```

From the SDK repository root:

```sh
export THRUPUT_CLI=/path/to/thruput-0.44.0
export CARGO_TARGET_DIR="$PWD/target/ristretto-msm"
cd experiments/ristretto-msm
"$THRUPUT_CLI" build --target-dir "$CARGO_TARGET_DIR" -- --locked
export RISTRETTO_MSM_RV32_ARTIFACT="$CARGO_TARGET_DIR/thruput/ristretto_msm.rv32a"
cargo test --locked --tests
```

The ignored tests check that a disabled executor returns `NoSyscall`. Point
`HOGD_FFI_LIB_DIR` at that build, then run:

```sh
cargo clean -p thruput-sys
cargo test --locked --tests -- --ignored
```
