use super::*;

#[test]
#[ignore = "requires network access"]
fn test_update_all_packages() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Install an older version
    ctx.depot()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // Update all packages
    ctx.depot().arg("update").assert().success();

    // Verify package is still installed (update should keep it)
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::exists());
}

#[test]
#[ignore = "requires network access"]
fn test_update_specific_package() {
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

    // Update specific package
    ctx.depot().arg("update").arg("penlight").assert().success();

    // Verify package is still installed
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::exists());
}
