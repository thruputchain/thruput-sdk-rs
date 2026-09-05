//! `impl_erc165!` must reject a claim for a trait the contract does not
//! implement: the claim is verified by the compiler, not trusted.

#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/*.rs");
}
