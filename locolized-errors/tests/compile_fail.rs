//! Compile-fail tests: the derive macro must reject invalid inputs with
//! precise, actionable diagnostics. Regenerate `.stderr` files with
//! `TRYBUILD=overwrite cargo test`.

#[test]
fn compile_fail() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
