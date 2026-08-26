//! Mock-executor misuse that must not compile.
//!
//! Separate from `compile_fail` because these cases need the `mock` feature;
//! without it the APIs are absent and the cases would fail to resolve rather
//! than fail on the bound under test.

#[test]
fn compile_fail_mock() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail-mock/*.rs");
}
