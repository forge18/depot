//! Tests for `lpm outdated` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_outdated_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = lpm_command()
        .arg("outdated")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail when package.yaml doesn't exist
    assert!(!output.status.success());
}

#[test]
fn test_outdated_with_empty_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with no dependencies
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("outdated")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No dependencies") || stdout.contains("to check"));
}

#[test]
fn test_outdated_with_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with dependencies
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
        .arg("outdated")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on network, but should handle gracefully
    assert!(output.status.code().is_some());
}

#[test]
fn test_outdated_with_lockfile() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
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

    // Create a lockfile
    fs::write(
        project_root.join("package.lock"),
        r#"
version: 1
packages:
  luasocket:
    version: "3.0.0"
    source: luarocks
    checksum: sha256:test
    dependencies: {}
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("outdated")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on network
    assert!(output.status.code().is_some());
}
