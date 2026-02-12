use super::*;
use depot::package::lockfile::Lockfile;

#[test]
#[ignore = "requires network access to download packages"]
fn install_creates_lua_modules() {
    TestContext::require_network();
    let ctx = TestContext::new();

    ctx.create_package_yaml(
        r#"
name: test-project
version: 1.0.0
dependencies:
  penlight: "1.13.1"
"#,
    );

    // Note: No timeout - let CI handle overall job timeout
    // This test is marked #[ignore = "reason"] so it won't run in fast test suite
    ctx.depot()
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Installing"))
        .stdout(predicate::str::contains("penlight"));

    // Verify installation
    use super::constants;
    ctx.temp
        .child(constants::LUA_MODULES)
        .assert(predicate::path::exists());
    ctx.temp
        .child(format!("{}/penlight", constants::LUA_MODULES))
        .assert(predicate::path::exists());
    ctx.temp
        .child(constants::PACKAGE_LOCK)
        .assert(predicate::path::exists());
}

#[test]
#[ignore = "requires network access to download packages"]
fn install_creates_lockfile_with_checksums() {
    TestContext::require_network();
    let ctx = TestContext::new();

    ctx.create_package_yaml(
        r#"
name: test
version: 1.0.0
dependencies:
  penlight: "1.13.1"
"#,
    );

    ctx.depot().arg("install").assert().success();

    // Verify lockfile structure by parsing it using Depot's actual parser
    // This verifies the structure matches the expected format instead of string matching
    let lockfile = Lockfile::load(ctx.temp.path())
        .expect("Failed to load lockfile")
        .expect("Lockfile should exist");

    // Verify lockfile structure
    assert_eq!(lockfile.version, 1, "Lockfile should have version 1");
    assert!(
        !lockfile.packages.is_empty(),
        "Lockfile should contain packages"
    );

    // Verify specific package exists with correct structure
    let package = lockfile
        .get_package("penlight")
        .expect("Lockfile should contain penlight");

    assert_eq!(package.version, "1.13.1", "Package version should match");
    assert!(!package.checksum.is_empty(), "Package should have checksum");
    assert_eq!(
        package.source, "luarocks",
        "Package source should be luarocks"
    );
}

#[test]
#[ignore = "requires network access to download packages"]
fn install_specific_package_adds_to_yaml() {
    TestContext::require_network();
    let ctx = TestContext::new();

    ctx.create_package_yaml("name: test\nversion: 1.0.0\n");

    ctx.depot()
        .arg("install")
        .arg("penlight@1.13.1")
        .assert()
        .success();

    // Verify package.yaml was updated
    use super::constants;
    let content = std::fs::read_to_string(ctx.temp.child(constants::PACKAGE_YAML).path()).unwrap();

    assert!(
        content.contains("penlight"),
        "Package not added to dependencies"
    );
}

#[test]
#[ignore = "requires network access to download packages"]
fn install_with_dev_flag() {
    TestContext::require_network();
    let ctx = TestContext::new();

    ctx.create_package_yaml("name: test\nversion: 1.0.0\n");

    ctx.depot()
        .arg("install")
        .arg("--dev")
        .arg("busted@2.0.0")
        .assert()
        .success();

    use super::constants;
    let content = std::fs::read_to_string(ctx.temp.child(constants::PACKAGE_YAML).path()).unwrap();

    assert!(
        content.contains("dev_dependencies") || content.contains("dev-dependencies"),
        "Package not added to dev dependencies"
    );
}

#[test]
fn install_without_package_yaml_fails() {
    let ctx = TestContext::new();

    ctx.depot().arg("install").assert().failure().stderr(
        predicate::str::contains("package.yaml")
            .or(predicate::str::contains("Could not find"))
            .or(predicate::str::contains("not found")),
    );
}

#[test]
#[ignore = "requires network access to verify package availability"]
fn install_with_invalid_version_fails() {
    TestContext::require_network();
    let ctx = TestContext::new();

    ctx.create_package_yaml("name: test\nversion: 1.0.0\n");

    ctx.depot()
        .arg("install")
        .arg("penlight@999.999.999")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("package.yaml not found")
                .or(predicate::str::contains("version")),
        );
}

#[test]
#[ignore = "requires network access to verify package availability"]
fn install_nonexistent_package_fails() {
    TestContext::require_network();
    let ctx = TestContext::new();

    ctx.create_package_yaml("name: test\nversion: 1.0.0\n");

    ctx.depot()
        .arg("install")
        .arg("nonexistent-package-xyz-12345")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("package.yaml not found")
                .or(predicate::str::contains("Package")),
        );
}
