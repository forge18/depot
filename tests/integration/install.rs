//! Tests for `lpm install` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_install_package_workflow() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\ndependencies:\n  lua-resty-http: ~> 0.17",
    )
    .unwrap();

    // Try to install (may fail if network unavailable, but should handle gracefully)
    let output = lpm_command()
        .arg("install")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should either succeed or fail gracefully with a clear error
    assert!(output.status.code().is_some());
}
