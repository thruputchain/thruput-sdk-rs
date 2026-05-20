mod common;
test_cfg!();

#[test]
fn compile_fail_storage() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail-storage/*.rs");
}
