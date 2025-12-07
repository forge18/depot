//! Tests for `lpm clean` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_clean_removes_lua_modules() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create lua_modules directory with some content
    let lua_modules = project_root.join("lua_modules");
    fs::create_dir_all(&lua_modules).unwrap();
    fs::write(lua_modules.join("test.lua"), "test").unwrap();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("clean")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success(), "lpm clean should succeed");
    assert!(!lua_modules.exists(), "lua_modules should be removed");
}

#[test]
fn test_clean_without_lua_modules() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("clean")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("does not exist") || stdout.contains("Nothing to clean"));
}

#[test]
fn test_clean_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = lpm_command()
        .arg("clean")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_clean_counts_packages() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create lua_modules with multiple packages
    let lua_modules = project_root.join("lua_modules");
    fs::create_dir_all(lua_modules.join("package1")).unwrap();
    fs::create_dir_all(lua_modules.join("package2")).unwrap();
    fs::create_dir_all(lua_modules.join(".lpm")).unwrap(); // Should be skipped

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("clean")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2") || stdout.contains("package"));
}
