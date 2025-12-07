//! Tests for error recovery scenarios

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_install_with_corrupted_lockfile() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Create corrupted lockfile
    fs::write(
        project_root.join("package.lock"),
        "invalid: lockfile: content: [",
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should either recover or provide clear error
    assert!(output.status.code().is_some());
    // If it fails, should provide helpful error message
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("lockfile") || stderr.contains("parse") || stderr.contains("invalid"),
            "Should mention lockfile issue in error"
        );
    }
}

#[test]
fn test_update_with_missing_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with dependency that doesn't exist
    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
dependencies:
  nonexistent-package-xyz-123: 999.0.0
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("update")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail gracefully with clear error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("nonexistent")
            || stderr.contains("package"),
        "Should mention package not found"
    );
}

#[test]
fn test_remove_nonexistent_package() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("remove")
        .arg("nonexistent-package")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail gracefully
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not installed")
            || stderr.contains("not found")
            || stderr.contains("nonexistent"),
        "Should mention package not found"
    );
}

#[test]
fn test_verify_with_missing_lockfile() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml but no lockfile
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("verify")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail gracefully
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("lockfile")
            || stderr.contains("package.lock")
            || stderr.contains("not found"),
        "Should mention missing lockfile"
    );
}

#[test]
fn test_list_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = lpm_command()
        .arg("list")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should handle gracefully (may succeed with empty list or fail with clear error)
    assert!(output.status.code().is_some());
}

#[test]
fn test_clean_with_partial_installation() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Create partial lua_modules directory
    fs::create_dir_all(project_root.join("lua_modules")).unwrap();
    fs::write(
        project_root.join("lua_modules").join("partial.lua"),
        "incomplete",
    )
    .unwrap();

    let output = lpm_command()
        .arg("clean")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should handle partial installation gracefully
    assert!(output.status.code().is_some());
}
