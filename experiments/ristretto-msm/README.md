# Ristretto255 MSM contract check

Checks the experimental host API through native and RV32 contract calls:
arithmetic, malformed inputs, the term limit and insufficient gas.

Use Thruput CLI 0.44.0 and native test FFI 0.44.1, which includes the MSM
host operation. The SDK downloads the pinned FFI by default; see
[native executor setup](../../crates/native-exec/README.md) for local builds.
The SDK pins protocol crates 0.66.0.

From the SDK repository root:

```sh
export THRUPUT_CLI=/path/to/thruput-0.44.0
export CARGO_TARGET_DIR="$PWD/target/ristretto-msm"
cd experiments/ristretto-msm
"$THRUPUT_CLI" build --target-dir "$CARGO_TARGET_DIR" -- --locked
export RISTRETTO_MSM_RV32_ARTIFACT="$CARGO_TARGET_DIR/thruput/ristretto_msm.rv32a"
cargo test --locked --tests
```
