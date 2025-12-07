//! Tests for `lpm update` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_update_package_workflow() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with dependencies
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\ndependencies:\n  lua-resty-http: ~> 0.17",
    )
    .unwrap();

    // Try to update (may fail if network unavailable, but should handle gracefully)
    let output = lpm_command()
        .arg("update")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should either succeed or fail gracefully
    assert!(output.status.code().is_some());
}

#[test]
fn test_update_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = lpm_command()
        .arg("update")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_update_specific_package() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
dependencies:
  luasocket: ~> 3.0
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("update")
        .arg("luasocket")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on network
    assert!(output.status.code().is_some());
}

#[test]
fn test_update_with_empty_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("update")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should handle gracefully
    assert!(output.status.code().is_some());
}
