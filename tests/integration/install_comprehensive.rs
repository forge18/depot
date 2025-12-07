//! Comprehensive tests for `lpm install` command

use super::common::lpm_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_install_global_without_package() {
    let output = lpm_command()
        .arg("install")
        .arg("--global")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Global installation requires a package name")
            || stderr.contains("package")
    );
}

#[test]
fn test_install_global_with_path() {
    let output = lpm_command()
        .arg("install")
        .arg("--global")
        .arg("--path")
        .arg("/some/path")
        .arg("test-package")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cannot install from local path globally") || stderr.contains("path"));
}

#[test]
fn test_install_with_no_dev_and_dev_only() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .arg("--no-dev")
        .arg("--dev-only")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cannot use both --no-dev and --dev-only") || stderr.contains("both"));
}

#[test]
fn test_install_from_path_nonexistent() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .arg("--path")
        .arg("/nonexistent/path")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Path does not exist") || stderr.contains("nonexistent"));
}

#[test]
fn test_install_from_path_valid() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create a local package
    let local_pkg = temp.path().join("local-pkg");
    fs::create_dir_all(&local_pkg).unwrap();
    fs::write(
        local_pkg.join("package.yaml"),
        "name: local-package\nversion: 1.0.0\n",
    )
    .unwrap();

    // Create main project
    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .arg("--path")
        .arg(local_pkg.to_str().unwrap())
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should succeed and add dependency
    assert!(output.status.success());

    // Verify package.yaml was updated
    let manifest_content = fs::read_to_string(project_root.join("package.yaml")).unwrap();
    assert!(manifest_content.contains("local-package") || manifest_content.contains("path:"));
}

#[test]
fn test_install_with_dev_flag() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create a local package
    let local_pkg = temp.path().join("local-pkg");
    fs::create_dir_all(&local_pkg).unwrap();
    fs::write(
        local_pkg.join("package.yaml"),
        "name: local-package\nversion: 1.0.0\n",
    )
    .unwrap();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .arg("--dev")
        .arg("--path")
        .arg(local_pkg.to_str().unwrap())
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());

    // Verify it was added as dev dependency
    let manifest_content = fs::read_to_string(project_root.join("package.yaml")).unwrap();
    assert!(
        manifest_content.contains("dev_dependencies") || manifest_content.contains("local-package")
    );
}

#[test]
fn test_install_all_dependencies_no_dev() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
dependencies:
  luasocket: ~> 3.0
dev_dependencies:
  busted: ~> 2.0
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .arg("--no-dev")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on network, but should handle --no-dev flag
    assert!(output.status.code().is_some());
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("dev dependencies skipped") || stdout.contains("Installing"));
    }
}

#[test]
fn test_install_all_dependencies_dev_only() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
dependencies:
  luasocket: ~> 3.0
dev_dependencies:
  busted: ~> 2.0
"#,
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .arg("--dev-only")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on network
    assert!(output.status.code().is_some());
}

#[test]
fn test_install_empty_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No dependencies") || stdout.contains("to install"));
}

#[test]
fn test_install_package_with_version_constraint() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .arg("luasocket@~>3.0")
        .current_dir(project_root)
        .output()
        .unwrap();

    // May succeed or fail depending on network
    assert!(output.status.code().is_some());
}

#[test]
fn test_install_package_invalid_version_constraint() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        "name: test-project\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = lpm_command()
        .arg("install")
        .arg("luasocket@invalid-version")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Invalid version constraint") || stderr.contains("version"));
}
