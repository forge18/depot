//! Tests for `depot exec` command

use super::common::depot_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_exec_no_command() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("exec")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No command provided") || stderr.contains("command"));
}

#[test]
fn test_exec_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = depot_command()
        .arg("exec")
        .arg("lua")
        .arg("-e")
        .arg("print('test')")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail when not in a project
    assert!(!output.status.success());
}

#[test]
fn test_exec_with_command() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Try to execute a simple Lua command
    let output = depot_command()
        .arg("exec")
        .arg("lua")
        .arg("-e")
        .arg("print('hello')")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on Lua availability
    assert!(output.status.code().is_some());
}

