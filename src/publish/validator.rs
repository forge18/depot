use crate::core::{LpmError, LpmResult};
use crate::package::manifest::PackageManifest;
use crate::package::validator::ManifestValidator;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Validates a package before publishing
pub struct PublishValidator;

impl PublishValidator {
    /// Validate a package is ready for publishing
    pub fn validate(manifest: &PackageManifest, project_root: &Path) -> LpmResult<()> {
        // 1. Validate manifest schema
        ManifestValidator::validate(manifest)?;

        // 2. Check that package has Lua files
        Self::validate_has_lua_files(project_root)?;

        // 3. Validate Rust build config if present
        if let Some(build) = &manifest.build {
            if build.build_type == "rust" {
                Self::validate_rust_build(project_root, build)?;
            }
        }

        // 4. Validate metadata
        Self::validate_metadata(manifest)?;

        println!("✓ Package validation passed");

        Ok(())
    }

    /// Check that the package contains Lua files
    fn validate_has_lua_files(project_root: &Path) -> LpmResult<()> {
        let lua_dirs = vec!["lua", "src", "lib", "."];
        let mut found_lua = false;

        for dir_name in lua_dirs {
            let dir = project_root.join(dir_name);
            if dir.exists() && dir.is_dir() {
                for entry in WalkDir::new(&dir) {
                    let entry = entry.map_err(|e| {
                        LpmError::Path(format!("Failed to read directory entry: {}", e))
                    })?;
                    let path = entry.path();
                    if path.is_file() && path.extension().map(|e| e == "lua").unwrap_or(false) {
                        found_lua = true;
                        break;
                    }
                }
            }

            // Also check root for .lua files
            if !found_lua {
                for entry in fs::read_dir(project_root)? {
                    let entry = entry.map_err(|e| {
                        LpmError::Path(format!("Failed to read directory entry: {}", e))
                    })?;
                    let path = entry.path();
                    if path.is_file() && path.extension().map(|e| e == "lua").unwrap_or(false) {
                        found_lua = true;
                        break;
                    }
                }
            }

            if found_lua {
                break;
            }
        }

        if !found_lua {
            return Err(LpmError::Package(
                "Package must contain at least one .lua file. LPM only publishes Lua modules, not standalone Rust libraries.".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate Rust build configuration
    fn validate_rust_build(
        project_root: &Path,
        build: &crate::package::manifest::BuildConfig,
    ) -> LpmResult<()> {
        // Check that Cargo.toml exists
        let cargo_toml = project_root.join(build.manifest.as_deref().unwrap_or("Cargo.toml"));

        if !cargo_toml.exists() {
            return Err(LpmError::Package(format!(
                "Cargo.toml not found at {}",
                cargo_toml.display()
            )));
        }

        // Check that modules are specified
        if build.modules.is_empty() {
            return Err(LpmError::Package(
                "Rust build must specify 'modules' mapping. Rust code must be compiled as native Lua modules, not standalone libraries.".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate package metadata
    fn validate_metadata(manifest: &PackageManifest) -> LpmResult<()> {
        if manifest.name.is_empty() {
            return Err(LpmError::Package(
                "Package name cannot be empty".to_string(),
            ));
        }

        if manifest.version.is_empty() {
            return Err(LpmError::Package(
                "Package version cannot be empty".to_string(),
            ));
        }

        // Description is recommended but not required
        if manifest.description.is_none() {
            println!("⚠️  Warning: Package has no description");
        }

        // License is recommended but not required
        if manifest.license.is_none() {
            println!("⚠️  Warning: Package has no license");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageManifest;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_manifest() -> PackageManifest {
        PackageManifest {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Test package".to_string()),
            homepage: None,
            license: Some("MIT".to_string()),
            lua_version: "5.4".to_string(),
            dependencies: std::collections::HashMap::new(),
            dev_dependencies: std::collections::HashMap::new(),
            scripts: std::collections::HashMap::new(),
            build: None,
            binary_urls: std::collections::HashMap::new(),
            resolution_strategy: None,
        }
    }

    #[test]
    fn test_validate_metadata_valid() {
        let manifest = create_test_manifest();
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();

        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_metadata_empty_name() {
        let mut manifest = create_test_manifest();
        manifest.name = String::new();
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();

        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_metadata_empty_version() {
        let mut manifest = create_test_manifest();
        manifest.version = String::new();
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();

        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_has_lua_files() {
        let manifest = create_test_manifest();
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();

        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_no_lua_files() {
        let manifest = create_test_manifest();
        let temp = TempDir::new().unwrap();
        // No .lua files

        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_rust_build_missing_cargo_toml() {
        let mut manifest = create_test_manifest();
        manifest.build = Some(crate::package::manifest::BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: vec![("test".to_string(), "lib.rs".to_string())]
                .into_iter()
                .collect(),
            features: Vec::new(),
            profile: None,
        });
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();

        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_rust_build_no_modules() {
        let mut manifest = create_test_manifest();
        manifest.build = Some(crate::package::manifest::BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: Vec::new(),
            profile: None,
        });
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_has_lua_files_in_src() {
        let manifest = create_test_manifest();
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("src")).unwrap();
        fs::write(temp.path().join("src").join("module.lua"), "return {}").unwrap();
        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_has_lua_files_in_lua() {
        let manifest = create_test_manifest();
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("lua")).unwrap();
        fs::write(temp.path().join("lua").join("module.lua"), "return {}").unwrap();
        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_rust_build_with_cargo_toml() {
        let mut manifest = create_test_manifest();
        manifest.build = Some(crate::package::manifest::BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: vec![("test".to_string(), "lib.rs".to_string())]
                .into_iter()
                .collect(),
            features: Vec::new(),
            profile: None,
        });
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();
        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_metadata_no_description() {
        let mut manifest = create_test_manifest();
        manifest.description = None;
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();
        // Should warn but not fail
        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_metadata_no_license() {
        let mut manifest = create_test_manifest();
        manifest.license = None;
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("test.lua"), "print('test')").unwrap();
        // Should warn but not fail
        let result = PublishValidator::validate(&manifest, temp.path());
        assert!(result.is_ok());
    }
}
