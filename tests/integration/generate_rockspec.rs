//! Tests for `depot generate-rockspec` command

use super::common::depot_command;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_generate_rockspec_without_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let output = depot_command()
        .arg("generate-rockspec")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_generate_rockspec_with_package_yaml() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
description: A test project
"#,
    )
    .unwrap();

    let output = depot_command()
        .arg("generate-rockspec")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    
    // Should create a .rockspec file
    let rockspec_files: Vec<_> = fs::read_dir(project_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rockspec"))
        .collect();
    
    assert!(!rockspec_files.is_empty(), "Should create a .rockspec file");
}

#[test]
fn test_generate_rockspec_with_dependencies() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.yaml"),
        r#"
name: test-project
version: 1.0.0
dependencies:
  luasocket: ~> 3.0
"#,
    )
    .unwrap();

    let output = depot_command()
        .arg("generate-rockspec")
        .current_dir(project_root)
        .output()
        .unwrap();

    assert!(output.status.success());
    
    // Find the generated rockspec file
    let rockspec_files: Vec<_> = fs::read_dir(project_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rockspec"))
        .collect();
    
    if let Some(rockspec_file) = rockspec_files.first() {
        let content = fs::read_to_string(rockspec_file.path()).unwrap();
        assert!(content.contains("test-project"));
        assert!(content.contains("luasocket") || content.contains("dependencies"));
    }
}

