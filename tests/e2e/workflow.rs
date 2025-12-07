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
    ctx.lpm().arg("init").arg("--yes").assert().success();

    // 2. Install a dependency
    println!("Step 2: Install dependency");
    ctx.lpm()
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
    ctx.lpm()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("penlight"));

    // 4. Verify checksums
    println!("Step 4: Verify checksums");
    ctx.lpm().arg("verify").assert().success();

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
        .expect("Failed to run lua");

    assert!(output.status.success());

    // 6. Remove the package
    println!("Step 6: Remove package");
    ctx.lpm().arg("remove").arg("penlight").assert().success();

    // 7. Verify removal
    println!("Step 7: Verify removal");
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::missing());

    println!("✓ Full workflow completed successfully");
}

#[test]
#[ignore = "requires Lua runtime to verify template functionality"]
fn test_template_to_working_project_love2d() {
    if !TestContext::has_lua() {
        TestContext::skip_test("Lua not available");
    }

    let ctx = TestContext::new();

    // Initialize with love2d template
    ctx.lpm()
        .arg("init")
        .arg("--template")
        .arg(constants::TEMPLATE_LOVE2D)
        .arg("--yes")
        .assert()
        .success();

    // Verify Love2D structure
    ctx.temp.child("main.lua").assert(predicate::path::exists());
    ctx.temp.child("conf.lua").assert(predicate::path::exists());

    // Verify main.lua has Love2D callbacks
    let main_content = std::fs::read_to_string(ctx.temp.child("main.lua").path()).unwrap();

    assert!(main_content.contains("love.load"), "Missing love.load");
    assert!(main_content.contains("love.update"), "Missing love.update");
    assert!(main_content.contains("love.draw"), "Missing love.draw");
}

#[test]
#[ignore = "requires network access for dev dependencies"]
fn test_dev_workflow() {
    TestContext::require_network();
    let ctx = TestContext::new();

    // Initialize
    ctx.lpm().arg("init").arg("--yes").assert().success();

    // Install production dependency
    ctx.lpm()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // Install dev dependency
    ctx.lpm()
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
    ctx.lpm().arg("clean").assert().success();

    ctx.lpm().arg("install").arg("--no-dev").assert().success();

    // Verify only production deps installed
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::exists());
    ctx.temp
        .child(format!("{}/busted", constants::LUA_MODULES))
        .assert(predicate::path::missing());
}
