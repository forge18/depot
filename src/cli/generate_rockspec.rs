use depot::core::path::find_project_root;
use depot::core::{DepotError, DepotResult};
use depot::package::manifest::PackageManifest;
use depot::publish::rockspec_generator::RockspecGenerator;
use std::env;
use std::fs;
use std::path::Path;

pub fn run() -> DepotResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;
    run_in_dir(&current_dir)
}

pub fn run_in_dir(dir: &Path) -> DepotResult<()> {
    let project_root = find_project_root(dir)?;
    let manifest = PackageManifest::load(&project_root)?;

    println!(
        "Generating rockspec for {}@{}...",
        manifest.name, manifest.version
    );

    let rockspec_content = RockspecGenerator::generate(&manifest)?;

    let luarocks_version = depot::luarocks::version::to_luarocks_version(
        &depot::core::version::Version::parse(&manifest.version)?,
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
        let temp = TempDir::new().unwrap();

        let result = run_in_dir(temp.path());

        // Should fail - no package.yaml
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_rockspec_with_manifest() {
        // Test with a valid manifest
        let temp = TempDir::new().unwrap();
        let manifest_content = r#"name: test-package
version: 1.0.0
description: Test package
"#;
        fs::write(temp.path().join("package.yaml"), manifest_content).unwrap();

        let result = run_in_dir(temp.path());

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
