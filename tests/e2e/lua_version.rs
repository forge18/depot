use super::*;

#[test]
#[ignore = "requires network access for Lua version management"]
fn test_lua_list() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // List available Lua versions
    ctx.lpm().arg("lua").arg("list").assert().success();
}

#[test]
#[ignore = "requires network access and may take a long time"]
fn test_lua_install() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Install a Lua version (using a stable version)
    ctx.lpm()
        .arg("lua")
        .arg("install")
        .arg("5.4")
        .assert()
        .success();
}

#[test]
#[ignore = "requires network access and Lua installation"]
fn test_lua_use() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // First install a Lua version
    ctx.lpm()
        .arg("lua")
        .arg("install")
        .arg("5.4")
        .assert()
        .success();

    // Switch to it
    ctx.lpm()
        .arg("lua")
        .arg("use")
        .arg("5.4")
        .assert()
        .success();
}
