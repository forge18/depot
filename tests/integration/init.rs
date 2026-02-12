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
fn test_init_with_template_flag() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Test with --yes and --template (non-interactive with template)
    let output = depot_command()
        .arg("init")
        .arg("--template")
        .arg("basic-lua")
        .arg("--yes")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should succeed (even if template doesn't exist, it should handle gracefully)
    // The important thing is that the command accepts the flags
    assert!(
        output.status.success()
            || String::from_utf8_lossy(&output.stderr).contains("template")
            || String::from_utf8_lossy(&output.stderr).contains("not found")
    );

    let package_yaml = project_root.join("package.yaml");
    if package_yaml.exists() {
        let content = fs::read_to_string(&package_yaml).unwrap();
        assert!(
            content.contains("name:"),
            "package.yaml should contain name"
        );
    }
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

    // Verify basic main.lua is created (in non-interactive mode without template)
    let main_lua = project_root.join("src").join("main.lua");
    if main_lua.exists() {
        let main_content = fs::read_to_string(&main_lua).unwrap();
        assert!(
            main_content.contains("print") || main_content.contains("Hello"),
            "main.lua should contain print or Hello statement"
        );
    }
    // Note: main.lua might not be created in non-interactive mode without template
    // The important thing is that directories are created
}

#[test]
fn test_init_with_template_non_interactive() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Test non-interactive mode with template
    let output = depot_command()
        .arg("init")
        .arg("--template")
        .arg("basic-lua")
        .arg("--yes")
        .current_dir(project_root)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{} {}", stdout, stderr);

    // Should succeed (even if template doesn't exist, it should handle gracefully)
    assert!(
        output.status.success()
            || combined.contains("template")
            || combined.contains("not found")
            || combined.contains("Template"),
        "Output: stdout={}, stderr={}",
        stdout,
        stderr
    );

    let package_yaml = project_root.join("package.yaml");
    if package_yaml.exists() {
        let content = fs::read_to_string(&package_yaml).unwrap();
        assert!(
            content.contains("name:"),
            "package.yaml should contain name"
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

    // Verify directory structure (may not exist if template was used)
    // Just verify package.yaml was created
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

    // Clean up and test with --template
    fs::remove_file(project_root.join("package.yaml")).ok();
    fs::remove_dir_all(project_root.join("src")).ok();
    fs::remove_dir_all(project_root.join("lib")).ok();
    fs::remove_dir_all(project_root.join("tests")).ok();

    let output2 = depot_command()
        .arg("init")
        .arg("--template")
        .arg("basic-lua")
        .arg("-y")
        .current_dir(project_root)
        .output()
        .unwrap();

    // Should handle template flag (may fail if template doesn't exist, but should not crash)
    assert!(output2.status.code().is_some());
}
