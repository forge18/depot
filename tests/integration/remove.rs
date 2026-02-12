//! Tests for `depot remove` command

use super::common::depot_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_remove_nonexistent_package() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("remove")
        .arg("nonexistent-package")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail or warn about missing package
    assert!(
        !output.status.success()
            || String::from_utf8_lossy(&output.stderr).contains("not found")
            || String::from_utf8_lossy(&output.stderr).contains("not in")
    );
}

#[test]
fn test_remove_package_workflow() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    // Try to remove a package (should handle gracefully if not installed)
    let output = depot_command()
        .arg("remove")
        .arg("nonexistent-package")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail or warn about missing package
    assert!(
        !output.status.success()
            || String::from_utf8_lossy(&output.stderr).contains("not found")
            || String::from_utf8_lossy(&output.stderr).contains("not in")
    );
}
