#[test]
fn trybuild_ui_tests() {
    let t = trybuild::TestCases::new();
    // Phase 1: happy path compiles. Phase 4 adds compile_fail cases for bad
    // bucket/dimension values.
    t.pass("tests/ui/happy_path.rs");
}
