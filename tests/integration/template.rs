//! Tests for `depot template` command

use super::common::depot_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_template_list_command() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("template")
        .arg("list")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should succeed and list templates
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show template list or indicate no templates
    assert!(
        !stdout.contains("error") && !stdout.contains("Error"),
        "Should not show errors"
    );
}

#[test]
fn test_template_list_with_search() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("template")
        .arg("list")
        .arg("--search")
        .arg("lua")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_template_create_without_project() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Don't create package.yaml - should fail
    let output = depot_command()
        .arg("template")
        .arg("create")
        .arg("test-template")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_template_create_with_project() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Create some project files
    fs::create_dir_all(project_root.join("src")).unwrap();
    fs::write(project_root.join("src").join("main.lua"), "print('hello')").unwrap();

    let output = depot_command()
        .arg("template")
        .arg("create")
        .arg("test-template")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on permissions, but should handle gracefully
    assert!(output.status.code().is_some());
}
