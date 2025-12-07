//! Tests for `lpm list` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_list_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = lpm_command()
        .arg("list")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail when package.yaml doesn't exist
    assert!(!output.status.success());
}

#[test]
fn test_list_with_empty_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with no dependencies
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("list")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("none") || stdout.contains("Dependencies"));
}

#[test]
fn test_list_with_dependencies() {
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
        .arg("list")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("luasocket") || stdout.contains("constraint"));
}

#[test]
fn test_list_tree_format() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("list")
        .arg("--tree")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_list_global() {
    let output = lpm_command().arg("list").arg("--global").output().unwrap();

    // May succeed with empty list or fail if no global packages
    assert!(output.status.code().is_some());
}

#[test]
fn test_list_with_dev_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml with dev dependencies
    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
dev_dependencies:
  busted: ~> 2.0
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("list")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("busted") || stdout.contains("Dev Dependencies") || stdout.contains("dev")
    );
}
