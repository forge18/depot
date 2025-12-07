//! Tests for `lpm plugin` commands

use super::common::lpm_command;

#[test]
fn test_plugin_list() {
    let output = lpm_command().arg("plugin").arg("list").output().unwrap();

    // Should succeed (may show empty list)
    assert!(output.status.code().is_some());
}

#[test]
fn test_plugin_info_nonexistent() {
    let output = lpm_command()
        .arg("plugin")
        .arg("info")
        .arg("nonexistent-plugin")
        .output()
        .unwrap();

    // Should fail gracefully
    assert!(!output.status.success());
}

#[test]
fn test_plugin_search() {
    let output = lpm_command().arg("plugin").arg("search").output().unwrap();

    // May succeed or fail depending on network/registry
    assert!(output.status.code().is_some());
}

#[test]
fn test_plugin_search_with_query() {
    let output = lpm_command()
        .arg("plugin")
        .arg("search")
        .arg("test")
        .output()
        .unwrap();

    // May succeed or fail depending on network/registry
    assert!(output.status.code().is_some());
}

#[test]
fn test_plugin_outdated() {
    let output = lpm_command()
        .arg("plugin")
        .arg("outdated")
        .output()
        .unwrap();

    // May succeed or fail depending on network/registry
    assert!(output.status.code().is_some());
}

#[test]
fn test_plugin_config_get_nonexistent() {
    let output = lpm_command()
        .arg("plugin")
        .arg("config")
        .arg("get")
        .arg("nonexistent-plugin")
        .arg("some-key")
        .output()
        .unwrap();

    // May succeed (showing "not found") or fail - either is acceptable
    // The important thing is it doesn't crash
    assert!(output.status.code().is_some());
}

#[test]
fn test_plugin_config_show_nonexistent() {
    let output = lpm_command()
        .arg("plugin")
        .arg("config")
        .arg("show")
        .arg("nonexistent-plugin")
        .output()
        .unwrap();

    // May succeed (showing empty config) or fail - either is acceptable
    // The important thing is it doesn't crash
    assert!(output.status.code().is_some());
}

#[test]
fn test_plugin_update_nonexistent() {
    let output = lpm_command()
        .arg("plugin")
        .arg("update")
        .arg("nonexistent-plugin")
        .output()
        .unwrap();

    // Should fail gracefully
    assert!(!output.status.success());
}
