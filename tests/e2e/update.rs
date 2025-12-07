use super::*;

#[test]
#[ignore = "requires network access"]
fn test_update_all_packages() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Install an older version
    ctx.lpm()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // Update all packages
    ctx.lpm().arg("update").assert().success();

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
    ctx.lpm()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // Update specific package
    ctx.lpm().arg("update").arg("penlight").assert().success();

    // Verify package is still installed
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::exists());
}
