use super::*;

#[test]
fn init_creates_package_yaml() {
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("init")
        .arg("--yes")
        .assert()
        .success()
        .stdout(super::test_predicates::contains_success_message());

    // Verify file exists and has correct structure
    use super::constants;
    let package_yaml = ctx.temp.child(constants::PACKAGE_YAML);
    package_yaml.assert(predicate::path::exists());

    let content = std::fs::read_to_string(package_yaml.path()).unwrap();
    assert!(content.contains("name:"), "Missing 'name' field");
    assert!(content.contains("version:"), "Missing 'version' field");
}

#[test]
fn init_with_template_basic_lua() {
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("init")
        .arg("--template")
        .arg(constants::TEMPLATE_BASIC_LUA)
        .arg("--yes")
        .assert()
        .success();

    // Verify template files created
    ctx.temp
        .child(constants::SRC_DIR)
        .assert(predicate::path::exists());
    ctx.temp
        .child(format!("{}/main.lua", constants::SRC_DIR))
        .assert(predicate::path::exists());
    ctx.temp
        .child(constants::PACKAGE_YAML)
        .assert(predicate::path::exists());
}

#[test]
fn init_with_template_love2d() {
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("init")
        .arg("--template")
        .arg(constants::TEMPLATE_LOVE2D)
        .arg("--yes")
        .assert()
        .success();

    ctx.temp.child("main.lua").assert(predicate::path::exists());
    ctx.temp.child("conf.lua").assert(predicate::path::exists());
}

#[test]
fn init_with_template_neovim_plugin() {
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("init")
        .arg("--template")
        .arg(constants::TEMPLATE_NEOVIM_PLUGIN)
        .arg("--yes")
        .assert()
        .success();

    ctx.temp.child("lua").assert(predicate::path::exists());
}

#[test]
fn init_with_template_lapis_web() {
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("init")
        .arg("--template")
        .arg(constants::TEMPLATE_LAPIS_WEB)
        .arg("--yes")
        .assert()
        .success();

    ctx.temp.child("app.lua").assert(predicate::path::exists());
    ctx.temp
        .child("nginx.conf")
        .assert(predicate::path::exists());
}

#[test]
fn init_with_template_cli_tool() {
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("init")
        .arg("--template")
        .arg(constants::TEMPLATE_CLI_TOOL)
        .arg("--yes")
        .assert()
        .success();

    ctx.temp
        .child(format!("{}/main.lua", constants::SRC_DIR))
        .assert(predicate::path::exists());
}

#[test]
fn init_fails_if_already_initialized() {
    let ctx = TestContext::new();
    ctx.create_package_yaml("name: existing\nversion: 1.0.0\n");

    ctx.lpm()
        .arg("init")
        .arg("--yes")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("already exists")
                .or(predicate::str::contains("Already in an LPM project")),
        );
}

#[test]
fn init_with_invalid_template_fails() {
    let ctx = TestContext::new();

    ctx.lpm()
        .arg("init")
        .arg("--template")
        .arg("nonexistent-template")
        .arg("--yes")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .or(predicate::str::contains("invalid template"))
                .or(predicate::str::contains("Template")),
        );
}
