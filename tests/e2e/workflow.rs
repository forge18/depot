use super::*;
use std::process::Command as StdCommand;

#[test]
#[ignore = "full workflow test, requires network access and Lua runtime"]
fn test_complete_project_workflow() {
    if !TestContext::has_lua() {
        TestContext::skip_test("Lua not available");
    }

    TestContext::require_network();
    let ctx = TestContext::new();

    // 1. Initialize new project
    println!("Step 1: Initialize project");
    ctx.depot().arg("init").arg("--yes").assert().success();

    // 2. Install a dependency
    println!("Step 2: Install dependency");
    ctx.depot()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // 3. Verify installation
    println!("Step 3: List packages");
    ctx.depot()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("penlight"));

    // 4. Verify checksums
    println!("Step 4: Verify checksums");
    ctx.depot().arg("verify").assert().success();

    // 5. Test the package works
    println!("Step 5: Test package functionality");
    let test_script = format!(
        r#"
{}
local pl = require('pl')
assert(pl.stringx.split("a,b", ",")[1] == "a")
print("Package works!")
"#,
        ctx.lua_package_path()
    );
    ctx.create_lua_script("test.lua", &test_script);

    let output = StdCommand::new("lua")
        .current_dir(&ctx.temp)
        .arg("test.lua")
        .output()
        .unwrap_or_else(|e| panic!("Failed to run lua for complete workflow test: {}", e));

    assert!(output.status.success());

    // 6. Remove the package
    println!("Step 6: Remove package");
    ctx.depot().arg("remove").arg("penlight").assert().success();

    // 7. Verify removal
    println!("Step 7: Verify removal");
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::missing());

    println!("✓ Full workflow completed successfully");
}

#[test]
#[ignore = "requires network access for dev dependencies"]
fn test_dev_workflow() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Initialize
    ctx.depot().arg("init").arg("--yes").assert().success();

    // Install production dependency
    ctx.depot()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // Install dev dependency
    ctx.depot()
        .arg("install")
        .arg("--dev")
        .arg(format!(
            "{}@{}",
            constants::PKG_BUSTED,
            constants::PKG_BUSTED_VERSION
        ))
        .assert()
        .success();

    // Verify both are installed
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::exists());
    ctx.temp
        .child(format!("{}/busted", constants::LUA_MODULES))
        .assert(predicate::path::exists());

    // Clean and install with --no-dev
    ctx.depot().arg("clean").assert().success();

    ctx.depot()
        .arg("install")
        .arg("--no-dev")
        .assert()
        .success();

    // Verify only production deps installed
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::exists());
    ctx.temp
        .child(format!("{}/busted", constants::LUA_MODULES))
        .assert(predicate::path::missing());
}
