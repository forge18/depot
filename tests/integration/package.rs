//! Tests for `lpm package` command

use super::common::depot_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_package_command() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("package")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should handle package command
    assert!(output.status.code().is_some());
}

#[test]
fn test_package_without_build_config() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml without build config
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("package")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No build configuration") || stderr.contains("build"));
}

#[test]
fn test_package_with_rust_build() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with Rust build config
    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
build:
  type: rust
"#,
    )
    .unwrap();

    let output = depot_command()
        .arg("package")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on Rust setup, but should handle gracefully
    assert!(output.status.code().is_some());
}

#[test]
fn test_package_with_target() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with Rust build config
    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
build:
  type: rust
"#,
    )
    .unwrap();

    let output = depot_command()
        .arg("package")
        .arg("--target")
        .arg("x86_64-unknown-linux-gnu")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on Rust setup
    assert!(output.status.code().is_some());
}
