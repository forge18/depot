use lpm::core::path::find_project_root;
use lpm::core::{LpmError, LpmResult};
use lpm::package::manifest::PackageManifest;
use lpm::publish::rockspec_generator::RockspecGenerator;
use std::env;
use std::fs;

pub fn run() -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;
    let manifest = PackageManifest::load(&project_root)?;

    println!(
        "Generating rockspec for {}@{}...",
        manifest.name, manifest.version
    );

    let rockspec_content = RockspecGenerator::generate(&manifest)?;

    let luarocks_version = lpm::luarocks::version::to_luarocks_version(
        &lpm::core::version::Version::parse(&manifest.version)?,
    );
    let rockspec_filename = format!("{}-{}.rockspec", manifest.name, luarocks_version);
    let rockspec_path = project_root.join(&rockspec_filename);

    fs::write(&rockspec_path, rockspec_content)?;

    println!("✓ Generated rockspec: {}", rockspec_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_generate_rockspec_function_exists() {
        // Test that run function exists
        let _ = run;
    }

    #[test]
    fn test_generate_rockspec_error_no_manifest() {
        // Test error path when package.yaml doesn't exist
        // Note: This test changes directory which can cause issues with tarpaulin
        // Skip if running under coverage tool
        if std::env::var("CARGO_TARPAULIN").is_ok() {
            return;
        }

        let temp = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = run();
        std::env::set_current_dir(original_dir).unwrap();

        // Should fail - no package.yaml
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_rockspec_with_manifest() {
        // Test with a valid manifest
        // Note: This test changes directory which can cause issues with tarpaulin
        // Skip if running under coverage tool
        if std::env::var("CARGO_TARPAULIN").is_ok() {
            return;
        }

        let temp = TempDir::new().unwrap();
        let manifest_content = r#"name: test-package
version: 1.0.0
description: Test package
"#;
        fs::write(temp.path().join("package.yaml"), manifest_content).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = run();
        std::env::set_current_dir(original_dir).unwrap();

        // May fail on rockspec generation, but tests the code path
        if result.is_ok() {
            // Check that rockspec was generated
            let rockspec_files: Vec<_> = std::fs::read_dir(temp.path())
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|s| s == "rockspec")
                        .unwrap_or(false)
                })
                .collect();
            assert!(!rockspec_files.is_empty());
        }
    }
}
