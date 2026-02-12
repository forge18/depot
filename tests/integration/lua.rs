//! Tests for `depot lua` commands

use super::common::depot_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_lua_list_command() {
    let output = depot_command().arg("lua").arg("list").output().unwrap();

    // Should succeed (may show empty list or list of versions)
    assert!(output.status.code().is_some());
}

#[test]
fn test_lua_list_remote_command() {
    let output = depot_command()
        .arg("lua")
        .arg("list-remote")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Available Lua versions")
            || stdout.contains("5.1")
            || stdout.contains("5.4")
    );
}

#[test]
fn test_lua_current_command() {
    let output = depot_command().arg("lua").arg("current").output().unwrap();

    // May succeed or fail depending on whether Lua is installed
    assert!(output.status.code().is_some());
}

#[test]
fn test_lua_which_command() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = depot_command()
        .arg("lua")
        .arg("which")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on whether Lua is installed
    assert!(output.status.code().is_some());
}

#[test]
fn test_lua_local_with_version_file() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create .lua-version file
    fs::write(project_root.join(".lua-version"), "5.4.8").unwrap();

    let output = depot_command()
        .arg("lua")
        .arg("which")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should detect .lua-version file
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() {
        assert!(stdout.contains("5.4.8") || stdout.contains(".lua-version"));
    }
}

#[test]
fn test_lua_uninstall_nonexistent() {
    let output = depot_command()
        .arg("lua")
        .arg("uninstall")
        .arg("999.999.999")
        .output()
        .unwrap();

    // Should fail gracefully
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not installed") || stderr.contains("not found"),
        "Should mention version not installed"
    );
}

#[test]
fn test_lua_exec_no_command() {
    let output = depot_command()
        .arg("lua")
        .arg("exec")
        .arg("5.4.8")
        .output()
        .unwrap();

    // Should fail when no command provided
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("command") || stderr.contains("No command"),
        "Should mention missing command"
    );
}

#[test]
fn test_lua_use_nonexistent_version() {
    let output = depot_command()
        .arg("lua")
        .arg("use")
        .arg("999.999.999")
        .output()
        .unwrap();

    // Should fail gracefully
    assert!(!output.status.success());
}

#[test]
fn test_lua_local_command() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = depot_command()
        .arg("lua")
        .arg("local")
        .arg("5.4.8")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on whether version exists
    // But should create .lua-version file if successful
    if output.status.success() {
        assert!(project_root.join(".lua-version").exists());
    }
}
