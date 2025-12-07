use super::*;

#[test]
#[ignore = "requires network access for global installation"]
fn test_global_install() {
    TestContext::require_network();
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("install")
        .arg("-g")
        .arg(format!(
            "{}@{}",
            constants::PKG_LUACHECK,
            constants::PKG_LUACHECK_VERSION
        ))
        .assert()
        .success();

    // Verify global installation directory exists
    // Note: Each test gets its own isolated global directory via TestContext
    let global_dir = ctx.lpm_home.join("global");
    assert!(global_dir.exists(), "Global directory not created");
}

#[test]
#[ignore = "requires network access for global installation"]
fn test_global_list() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Install a global package first
    ctx.lpm()
        .arg("install")
        .arg("-g")
        .arg(format!(
            "{}@{}",
            constants::PKG_LUACHECK,
            constants::PKG_LUACHECK_VERSION
        ))
        .assert()
        .success();

    // List global packages
    ctx.lpm()
        .arg("list")
        .arg("--global")
        .assert()
        .success()
        .stdout(predicate::str::contains("luacheck"));
}

#[test]
#[ignore = "requires network access for global installation"]
fn test_global_remove() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Install then remove
    ctx.lpm()
        .arg("install")
        .arg("-g")
        .arg(format!(
            "{}@{}",
            constants::PKG_LUACHECK,
            constants::PKG_LUACHECK_VERSION
        ))
        .assert()
        .success();

    ctx.lpm()
        .arg("remove")
        .arg("-g")
        .arg("luacheck")
        .assert()
        .success();

    // Verify it's gone
    ctx.lpm()
        .arg("list")
        .arg("--global")
        .assert()
        .success()
        .stdout(predicate::str::contains("luacheck").not());
}
