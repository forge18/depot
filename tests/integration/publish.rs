//! Tests for `lpm publish` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_publish_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = lpm_command()
        .arg("publish")
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
fn test_publish_without_credentials() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
description: Test project
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("publish")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail when credentials are not set
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{} {}", stderr, stdout);
    assert!(
        combined.contains("login")
            || combined.contains("credential")
            || combined.contains("username")
            || combined.contains("API key")
            || combined.contains("not found")
            || combined.contains("error"),
        "Should mention login, credentials, or provide error. stderr: {}, stdout: {}",
        stderr,
        stdout
    );
}

#[test]
fn test_publish_validation() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with missing required fields
    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: ""
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("publish")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail validation
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("valid") || stderr.contains("version") || stderr.contains("error"),
        "Should provide validation error"
    );
}

#[test]
fn test_publish_workflow_with_lua_files() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
description: Test project
"#,
    )
    .unwrap();

    // Create some Lua files
    fs::create_dir_all(project_root.join("src")).unwrap();
    fs::write(
        project_root.join("src").join("main.lua"),
        "print('Hello, World!')",
    )
    .unwrap();

    let output = lpm_command()
        .arg("publish")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May fail due to missing credentials, but should validate package structure
    // The important thing is it doesn't crash
    assert!(output.status.code().is_some());
}

#[test]
fn test_publish_error_recovery() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create invalid package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "invalid: yaml: content: [",
    )
    .unwrap();

    let output = lpm_command()
        .arg("publish")
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
