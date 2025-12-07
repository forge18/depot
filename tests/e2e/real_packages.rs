use super::*;
use std::process::Command as StdCommand;

/// Well-known stable packages to test with
/// Format: (package_name, version, require_name)
const STABLE_PACKAGES: &[(&str, &str, &str)] = &[
    (
        constants::PKG_PENLIGHT,
        constants::PKG_PENLIGHT_VERSION,
        "pl",
    ),
    (
        constants::PKG_DKJSON,
        constants::PKG_DKJSON_VERSION,
        "dkjson",
    ),
    (
        constants::PKG_LUASOCKET,
        constants::PKG_LUASOCKET_VERSION,
        "socket",
    ),
    (
        constants::PKG_LUAFILESYSTEM,
        constants::PKG_LUAFILESYSTEM_VERSION,
        "lfs",
    ),
];

#[test]
#[ignore = "requires network access and Lua runtime to test real packages"]
fn test_all_stable_packages() {
    if !TestContext::has_lua() {
        TestContext::skip_test("Lua not available on PATH");
    }

    TestContext::require_network();

    for (name, version, require_name) in STABLE_PACKAGES {
        println!("\n=== Testing package: {} @ {} ===", name, version);
        test_package_installation_and_loading(name, version, require_name);
    }
}

fn test_package_installation_and_loading(name: &str, version: &str, require_name: &str) {
    let ctx = TestContext::new();

    // Install package
    println!("  Installing {}@{}...", name, version);
    let result = ctx
        .lpm()
        .arg("install")
        .arg(format!("{}@{}", name, version))
        .output();

    match result {
        Ok(output) if output.status.success() => {
            // Installation succeeded
        }
        Ok(output) => {
            // Installation failed - check if it's a version unavailable error
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not found") || stderr.contains("No versions available") {
                TestContext::handle_unavailable_version(name, version, &stderr);
            } else {
                panic!("Installation failed: {}", stderr);
            }
        }
        Err(e) => {
            panic!("Failed to run lpm install: {}", e);
        }
    }

    // Verify installation
    println!("  Verifying installation...");
    ctx.temp
        .child(format!("{}/{}", constants::LUA_MODULES, name))
        .assert(predicate::path::exists());

    // Create test script using helper for path setup
    let test_script = format!(
        r#"
-- Add lua_modules to path
{}

-- Try to load the module
local ok, result = pcall(require, '{}')

if not ok then
    print("ERROR: Failed to require '{}'")
    print("Error message: " .. tostring(result))
    os.exit(1)
end

-- Verify we got something back
if result == nil then
    print("ERROR: require('{}') returned nil")
    os.exit(1)
end

print("SUCCESS: {} loaded successfully")
print("Module type: " .. type(result))
"#,
        ctx.lua_path_setup(),
        require_name,
        require_name,
        require_name,
        name
    );

    ctx.create_lua_script("test.lua", &test_script);

    // Run test script
    println!("  Testing with Lua runtime...");
    let output = StdCommand::new("lua")
        .current_dir(&ctx.temp)
        .arg("test.lua")
        .output()
        .expect("Failed to run lua");

    assert!(output.status.success(), "Lua script failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SUCCESS"), "Script output missing SUCCESS");

    println!("  ✓ {} passed all tests", name);
}

#[test]
#[ignore = "requires network access and Lua runtime"]
fn test_penlight_functionality() {
    if !TestContext::has_lua() {
        TestContext::skip_test("Lua not available");
    }

    TestContext::require_network();
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_PENLIGHT,
            constants::PKG_PENLIGHT_VERSION
        ))
        .assert()
        .success();

    // Test actual penlight functionality
    let test_script = format!(
        r#"
{}

local stringx = require('pl.stringx')
local tablex = require('pl.tablex')

-- Test string splitting
local result = stringx.split("hello,world,test", ",")
assert(#result == 3, "split failed: expected 3 parts")
assert(result[1] == "hello", "split failed: wrong first element")
assert(result[2] == "world", "split failed: wrong second element")
assert(result[3] == "test", "split failed: wrong third element")

-- Test string stripping
local stripped = stringx.strip("  hello  ")
assert(stripped == "hello", "strip failed")

-- Test table operations
local t = {{a=1, b=2, c=3}}
local keys = tablex.keys(t)
assert(#keys == 3, "keys failed")

print("SUCCESS: All penlight functions work correctly!")
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SUCCESS"));
}

#[test]
#[ignore = "requires network access and Lua runtime"]
fn test_package_with_dependencies() {
    if !TestContext::has_lua() {
        TestContext::skip_test("Lua not available");
    }

    TestContext::require_network();
    let ctx = TestContext::new();

    // busted has multiple dependencies
    ctx.lpm()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_BUSTED,
            constants::PKG_BUSTED_VERSION
        ))
        .assert()
        .success();

    // Verify all dependencies were installed
    ctx.temp
        .child(format!("{}/busted", constants::LUA_MODULES))
        .assert(predicate::path::exists());
    ctx.temp
        .child(format!("{}/luassert", constants::LUA_MODULES))
        .assert(predicate::path::exists());
    ctx.temp
        .child(format!("{}/say", constants::LUA_MODULES))
        .assert(predicate::path::exists());

    // Verify transitive dependencies work
    let test_script = format!(
        r#"
{}

local busted = require('busted')
local assert = require('luassert')
local say = require('say')

assert(busted ~= nil, "busted not loaded")
assert(type(assert) == "table", "luassert not loaded correctly")
assert(say ~= nil, "say not loaded")

print("SUCCESS: All dependencies loaded correctly!")
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
}

#[test]
#[ignore = "requires network access and Lua runtime for C extensions"]
fn test_c_extension_package() {
    if !TestContext::has_lua() {
        TestContext::skip_test("Lua not available");
    }

    TestContext::require_network();
    let ctx = TestContext::new();

    // luasocket is a C extension
    ctx.lpm()
        .arg("install")
        .arg(format!(
            "{}@{}",
            constants::PKG_LUASOCKET,
            constants::PKG_LUASOCKET_VERSION
        ))
        .assert()
        .success();

    let test_script = format!(
        r#"
{}

local socket = require('socket')

-- Verify core socket functions exist
assert(socket.bind ~= nil, "socket.bind missing")
assert(socket.connect ~= nil, "socket.connect missing")
assert(socket.gettime ~= nil, "socket.gettime missing")

-- Test gettime function
local time = socket.gettime()
assert(type(time) == "number", "gettime didn't return number")
assert(time > 0, "gettime returned invalid time")

print("SUCCESS: luasocket C extension works!")
"#,
        ctx.lua_path_setup()
    );

    ctx.create_lua_script("test.lua", &test_script);

    let output = StdCommand::new("lua")
        .current_dir(&ctx.temp)
        .arg("test.lua")
        .output()
        .expect("Failed to run lua");

    assert!(output.status.success());
}
