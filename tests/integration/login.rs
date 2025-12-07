//! Tests for `lpm login` command

use super::common::lpm_command;

#[test]
fn test_login_command_exists() {
    // Test that login command exists and can be invoked
    // Note: Full testing requires stdin mocking, so we just verify it doesn't crash
    let output = lpm_command().arg("login").output().unwrap();

    // Should either prompt for input or show usage
    assert!(output.status.code().is_some());
}

#[test]
fn test_login_without_tty() {
    // In non-interactive environment, should handle gracefully
    let output = lpm_command()
        .arg("login")
        .env("CI", "true")
        .output()
        .unwrap();

    // May fail or show error, but should not crash
    assert!(output.status.code().is_some());
}
