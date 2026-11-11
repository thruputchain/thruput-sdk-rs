# Ristretto255 MSM contract check

Checks the experimental host API through native and RV32 contract calls:
arithmetic, malformed inputs, the term limit and insufficient gas.

Use Thruput CLI 0.44.0 and
[hogd `0e1d7662`](https://github.com/thruputchain/hogd/commit/0e1d7662341001783662e9159ebc248ce11611bd)
with the MSM host API. Set `HOGD_FFI_LIB_DIR` to a directory
containing that build's `libtest_ffi` shared library and `bindings.rs`; see
[native executor setup](../../crates/native-exec/README.md).
The manifest pins the matching protocol bindings.

After building hogd's `test_ffi` target in `build-msm`, generate its bindings
from the hogd checkout:

```sh
bash hogd/test_ffi/run_bindgen.sh build-msm/hogd/test_ffi/bindings.rs
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
