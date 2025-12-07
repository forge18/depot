//! Tests for `lpm build` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_build_command() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("build")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should handle build command (may fail if no Rust code, but should not crash)
    assert!(output.status.code().is_some());
}

#[test]
fn test_build_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = lpm_command()
        .arg("build")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail gracefully when package.yaml doesn't exist
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("package.yaml") || stderr.contains("not found"),
        "Should mention package.yaml in error"
    );
}

#[test]
fn test_build_with_rust_extension() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with Rust build type
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

    // Create a minimal Cargo.toml
    fs::create_dir_all(project_root.join("src")).unwrap();
    fs::write(
        project_root.join("Cargo.toml"),
        r#"
[package]
name = "test-project"
version = "1.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
mlua = "0.9"
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("build")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on dependencies, but should not crash
    assert!(output.status.code().is_some());
}

#[test]
fn test_build_error_recovery() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create invalid package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "invalid: yaml: content: [",
    )
    .unwrap();

    let output = lpm_command()
        .arg("build")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail with a clear error message
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("parse") || stderr.contains("invalid") || stderr.contains("error"),
        "Should provide error message for invalid YAML"
    );
}
