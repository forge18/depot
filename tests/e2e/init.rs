use super::*;

#[test]
fn init_creates_package_yaml() {
    let ctx = TestContext::new();

    ctx.depot()
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
fn init_fails_if_already_initialized() {
    let ctx = TestContext::new();
    ctx.create_package_yaml("name: existing\nversion: 1.0.0\n");

    ctx.depot()
        .arg("init")
        .arg("--yes")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("already exists")
                .or(predicate::str::contains("Already in an Depot project")),
        );
}
