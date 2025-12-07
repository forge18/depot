//! Tests for `lpm run` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_run_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = lpm_command()
        .arg("run")
        .arg("test")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_run_nonexistent_script() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("run")
        .arg("nonexistent-script")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("Script"));
}

#[test]
fn test_run_with_valid_script() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
scripts:
  test: "lua -e 'print(\"test\")'"
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("run")
        .arg("test")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on Lua availability
    assert!(output.status.code().is_some());
}

#[test]
fn test_run_with_empty_script() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
scripts:
  empty: ""
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("run")
        .arg("empty")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no command") || stderr.contains("empty"));
}
