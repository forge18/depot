use super::*;

#[test]
#[ignore = "requires network access"]
fn test_list_installed_packages() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Install a package
    ctx.depot()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // List packages
    ctx.depot()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("penlight"));
}

#[test]
#[ignore = "requires network access"]
fn test_list_with_tree() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Install a package with dependencies
    ctx.depot()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_BUSTED,
            constants::PKG_BUSTED_VERSION
        ))
        .assert()
        .success();

    // List with tree view
    ctx.depot()
        .arg("list")
        .arg("--tree")
        .assert()
        .success()
        .stdout(predicate::str::contains("busted"));
}
