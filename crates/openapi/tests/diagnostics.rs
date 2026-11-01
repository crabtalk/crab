#[test]
fn unsupported_contracts_report_the_selected_operation() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
