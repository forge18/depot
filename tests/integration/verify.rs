//! Tests for `depot verify` command

use super::common::depot_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_verify_with_no_lockfile() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("verify")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should handle missing lockfile gracefully
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No package.lock found") || stderr.contains("lockfile"));
}

#[test]
fn test_verify_with_empty_lockfile() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Create a minimal lockfile
    fs::write(
        project_root.join("package.lock"),
        "version: 1\ngenerated_at: 2024-01-01T00:00:00Z\npackages: {}\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("verify")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No packages") || stdout.contains("verified"));
}

#[test]
fn test_verify_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = depot_command()
        .arg("verify")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
}
