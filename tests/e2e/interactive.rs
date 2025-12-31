// Interactive tests using rexpect - Unix only (PTY not available on Windows)
#![cfg(unix)]

use super::*;
use rexpect::spawn;

#[test]
#[ignore = "requires terminal interaction, may be flaky in CI"]
fn test_interactive_install_basic() {
    TestContext::require_network();
    TestContext::require_tty(); // Skip if no TTY (e.g., in CI)

    let ctx = TestContext::new();

    ctx.create_package_yaml("name: test\nversion: 1.0.0\n");

    let lpm_path = env!("CARGO_BIN_EXE_lpm");
    let mut session = spawn(&format!("{} install --interactive", lpm_path), Some(10000))
        .expect("Failed to spawn lpm");

    // Wait for search prompt
    session
        .exp_string("Search")
        .expect("Search prompt not found");

    // Type package name
    session.send_line("penlight").expect("Failed to send input");

    // Wait for results and select first
    session.exp_string("penlight").expect("Results not shown");
    session.send_line("").expect("Failed to select");

    // Wait for version selection and select first (latest)
    session
        .exp_string("version")
        .expect("Version prompt not found");
    session.send_line("").expect("Failed to select version");

    // Wait for dependency type and select production
    session
        .exp_string("Install as")
        .expect("Dependency type prompt not found");
    session.send_line("").expect("Failed to select type");

    // Wait for success
    session
        .exp_string("Installing")
        .expect("Installation not started");

    // Give it time to complete
    std::thread::sleep(std::time::Duration::from_secs(30));
}

#[test]
#[ignore = "requires terminal interaction"]
fn test_init_wizard() {
    TestContext::require_tty(); // Skip if no TTY (e.g., in CI)

    let ctx = TestContext::new();

    let lpm_path = env!("CARGO_BIN_EXE_lpm");
    let mut session =
        spawn(&format!("{} init", lpm_path), Some(10000)).expect("Failed to spawn lpm");

    // Project name
    session
        .exp_string("Project name")
        .expect("Name prompt not found");
    session
        .send_line("my-test-project")
        .expect("Failed to send name");

    // Version
    session
        .exp_string("Version")
        .expect("Version prompt not found");
    session.send_line("1.0.0").expect("Failed to send version");

    // Description
    session
        .exp_string("Description")
        .expect("Description prompt not found");
    session
        .send_line("Test project")
        .expect("Failed to send description");

    // Should complete and create package.yaml
    session
        .exp_string("Created")
        .expect("Success message not found");

    // Verify file was created
    use super::constants;
    ctx.temp
        .child(constants::PACKAGE_YAML)
        .assert(predicate::path::exists());
}
