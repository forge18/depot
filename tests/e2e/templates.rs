use super::*;

#[test]
fn test_template_list() {
    let ctx = TestContext::new();

    // List available templates
    ctx.depot()
        .arg("template")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("basic-lua").or(predicate::str::contains("love2d")));
}

#[test]
#[ignore = "requires write access to template directory"]
fn test_template_create() {
    let ctx = TestContext::new();

    // Create a custom template
    // This test may need adjustment based on actual template creation command
    ctx.depot()
        .arg("template")
        .arg("create")
        .arg("test-template")
        .arg("--from")
        .arg("basic-lua")
        .assert()
        .success();
}
