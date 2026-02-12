use super::*;

#[test]
#[ignore = "requires network access"]
fn test_remove_package() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // First install a package
    ctx.depot()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // Verify it's installed
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::exists());

    // Remove it
    ctx.depot().arg("remove").arg("penlight").assert().success();

    // Verify it's gone
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::missing());
}

#[test]
fn test_remove_nonexistent_package_fails() {
    let ctx = TestContext::new();

    // Create package.yaml first (required for remove command)
    ctx.create_package_yaml("name: test\nversion: 1.0.0\n");

    ctx.depot()
        .arg("remove")
        .arg("nonexistent-package")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .or(predicate::str::contains("No such package"))
                .or(predicate::str::contains("not installed")),
        );
}
