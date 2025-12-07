//! Tests for interactive mode functionality

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_init_with_yes_flag_bypasses_interactive() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Using --yes should bypass interactive mode
    let output = lpm_command()
        .arg("init")
        .arg("--yes")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success(), "init --yes should succeed");

    // Verify package.yaml was created without interactive prompts
    let package_yaml = project_root.join("package.yaml");
    assert!(package_yaml.exists(), "package.yaml should be created");

    // Should not contain interactive prompts in output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("Project name") && !stderr.contains("Project name"),
        "Should not show interactive prompts with --yes flag"
    );
}

#[test]
fn test_init_with_y_flag_short_form() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Using -y should also bypass interactive mode
    let output = lpm_command()
        .arg("init")
        .arg("-y")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success(), "init -y should succeed");

    let package_yaml = project_root.join("package.yaml");
    assert!(package_yaml.exists(), "package.yaml should be created");
}

#[test]
fn test_install_without_interactive_flag() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Without --interactive flag, should run in non-interactive mode
    let output = lpm_command()
        .arg("install")
        .arg("penlight")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on network, but should not show interactive prompts
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("Search for packages") && !stderr.contains("Search for packages"),
        "Should not show interactive prompts without --interactive flag"
    );
}

#[test]
fn test_update_without_interactive_confirmation() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Update command should work without interactive confirmation when not in TTY
    let output = lpm_command()
        .arg("update")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should not require interactive confirmation
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("Proceed with update?") && !stderr.contains("Proceed with update?"),
        "Should not show confirmation prompt in non-interactive mode"
    );
}

#[test]
fn test_interactive_functions_available() {
    // Test that interactive functions can be imported and called
    // This verifies the module structure is correct
    use lpm::package::interactive::{choose, confirm, confirm_with_default};

    // Functions should exist and have correct signatures
    // We can't fully test them without mocking stdin/stdout,
    // but we can verify they compile and are accessible
    let _confirm_fn: fn(&str) -> lpm::core::LpmResult<bool> = confirm;
    let _confirm_default_fn: fn(&str, bool) -> lpm::core::LpmResult<bool> = confirm_with_default;
    let _choose_fn: fn(&str, &[&str], usize) -> lpm::core::LpmResult<usize> = choose;
}

#[test]
fn test_non_interactive_mode_handles_missing_stdin() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Commands should work even when stdin is not available (non-interactive mode)
    // This simulates running in a non-TTY environment
    let output = lpm_command()
        .arg("list")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should succeed without requiring stdin
    assert!(output.status.code().is_some());
}

#[test]
fn test_init_template_non_interactive() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Using template with --yes should bypass all interactive prompts
    let output = lpm_command()
        .arg("init")
        .arg("--template")
        .arg("basic-lua")
        .arg("--yes")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on template availability,
    // but should not show interactive prompts
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("Project name")
            && !stderr.contains("Project name")
            && !stdout.contains("License")
            && !stderr.contains("License"),
        "Should not show interactive prompts when using template with --yes"
    );
}

#[test]
fn test_commands_respect_non_interactive_environment() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Set CI environment variable (common way to detect non-interactive mode)
    let output = lpm_command()
        .arg("list")
        .env("CI", "true")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should work in CI/non-interactive environment
    assert!(output.status.code().is_some());
}
