//! Tests for `depot init` command

use super::common::depot_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_init_creates_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Use --yes flag for non-interactive mode
    let output = depot_command()
        .arg("init")
        .arg("--yes")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success(), "depot init --yes should succeed");

    let package_yaml = project_root.join("package.yaml");
    assert!(package_yaml.exists(), "package.yaml should be created");

    let content = fs::read_to_string(&package_yaml).unwrap();
    assert!(
        content.contains("name:"),
        "package.yaml should contain name"
    );
    assert!(
        content.contains("version:"),
        "package.yaml should contain version"
    );
}

#[test]
fn test_init_with_existing_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create existing package.yaml
    fs::write(
        project_root.join("package.yaml"),
        "name: existing\nversion: 1.0.0\n",
    )
    .unwrap();

    let output = depot_command()
        .arg("init")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should fail or warn about existing file
    // The exact behavior depends on implementation
    assert!(
        !output.status.success()
            || String::from_utf8_lossy(&output.stderr).contains("exists")
            || String::from_utf8_lossy(&output.stderr).contains("already")
    );
}

#[test]
fn test_init_non_interactive_mode() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = depot_command()
        .arg("init")
        .arg("--yes")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success(), "depot init --yes should succeed");

    let package_yaml = project_root.join("package.yaml");
    assert!(package_yaml.exists(), "package.yaml should be created");

    let content = fs::read_to_string(&package_yaml).unwrap();
    assert!(
        content.contains("name:"),
        "package.yaml should contain name"
    );
    assert!(
        content.contains("version:"),
        "package.yaml should contain version"
    );

    // Verify directory structure is created
    assert!(
        project_root.join("src").exists(),
        "src/ directory should be created"
    );
    assert!(
        project_root.join("lib").exists(),
        "lib/ directory should be created"
    );
    assert!(
        project_root.join("tests").exists(),
        "tests/ directory should be created"
    );
}

#[test]
fn test_init_creates_basic_structure() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = depot_command()
        .arg("init")
        .arg("--yes")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());

    // Verify package.yaml exists
    let package_yaml = project_root.join("package.yaml");
    assert!(package_yaml.exists());

    // Verify basic directory structure
    assert!(
        project_root.join("src").is_dir(),
        "src/ directory should be created"
    );
    assert!(
        project_root.join("lib").is_dir(),
        "lib/ directory should be created"
    );
    assert!(
        project_root.join("tests").is_dir(),
        "tests/ directory should be created"
    );

    // Verify basic main.lua is created
    let main_lua = project_root.join("src").join("main.lua");
    if main_lua.exists() {
        let main_content = fs::read_to_string(&main_lua).unwrap();
        assert!(
            main_content.contains("print") || main_content.contains("Hello"),
            "main.lua should contain print or Hello statement"
        );
    }
}

#[test]
fn test_init_non_interactive_creates_structure() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = depot_command()
        .arg("init")
        .arg("--yes")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "depot init --yes should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify package.yaml exists
    let package_yaml = project_root.join("package.yaml");
    assert!(package_yaml.exists(), "package.yaml should be created");

    // Verify package.yaml was created
    let content = fs::read_to_string(&package_yaml).unwrap();
    assert!(
        content.contains("name:"),
        "package.yaml should contain name"
    );
}

#[test]
fn test_init_with_all_flags() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Test --yes flag
    let output1 = depot_command()
        .arg("init")
        .arg("-y")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output1.status.success());
}
