#[test]
fn branch_and_call_consumers_reject_incompatible_contracts() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/workflows/*.rs");
    tests.compile_fail("tests/ui/*.rs");
}
