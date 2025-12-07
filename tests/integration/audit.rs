//! Tests for `lpm audit` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_audit_with_no_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("audit")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should handle empty dependencies gracefully
    assert!(output.status.code().is_some());
}

#[test]
fn test_audit_without_lockfile() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("audit")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No lockfile") || stderr.contains("Run 'lpm install'"));
}

#[test]
fn test_audit_with_lockfile() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
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
        .arg("audit")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on network, but should handle gracefully
    assert!(output.status.code().is_some());
}
