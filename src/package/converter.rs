use crate::core::path::{ensure_dir, packages_metadata_dir};
use crate::core::{DepotError, DepotResult};
use crate::luarocks::rockspec::Rockspec;
use crate::package::manifest::PackageManifest;
use std::fs;
use std::path::Path;

/// Convert a rockspec to package.yaml format and save it
pub fn convert_rockspec_to_manifest(
    rockspec: &Rockspec,
    project_root: &Path,
    package_name: &str,
) -> DepotResult<PackageManifest> {
    // Convert rockspec to manifest
    let manifest = rockspec.to_package_manifest();

    // Save to lua_modules/.depot/packages/<name>/package.yaml
    let packages_dir = packages_metadata_dir(project_root);
    let package_metadata_dir = packages_dir.join(package_name);
    ensure_dir(&package_metadata_dir)?;

    let manifest_path = package_metadata_dir.join("package.yaml");
    let yaml_content = serde_yaml::to_string(&manifest)
        .map_err(|e| DepotError::Package(format!("Failed to serialize manifest: {}", e)))?;

    fs::write(&manifest_path, yaml_content)?;

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_convert_rockspec() {
        let temp = TempDir::new().unwrap();
        let rockspec = Rockspec {
            package: "luasocket".to_string(),
            version: "3.0-1".to_string(),
            source: RockspecSource {
                url: "https://github.com/lunarmodules/luasocket/archive/v3.0.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec!["lua >= 5.1".to_string()],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: {
                    let mut m = HashMap::new();
                    m.insert("socket".to_string(), "src/socket.lua".to_string());
                    m
                },
                install: InstallTable::default(),
            },
            description: Some("Network support for Lua".to_string()),
            homepage: Some("https://github.com/lunarmodules/luasocket".to_string()),
            license: Some("MIT".to_string()),
            lua_version: Some(">=5.1".to_string()),
            binary_urls: HashMap::new(),
        };

        let manifest = convert_rockspec_to_manifest(&rockspec, temp.path(), "luasocket").unwrap();

        assert_eq!(manifest.name, "luasocket");
        assert_eq!(manifest.version, "3.0-1");
        assert_eq!(manifest.dependencies.len(), 1);
        assert!(manifest.dependencies.contains_key("lua"));

        // Verify file was created
        let manifest_path = temp
            .path()
            .join("lua_modules")
            .join(".depot")
            .join("packages")
            .join("luasocket")
            .join("package.yaml");
        assert!(manifest_path.exists());
    }

    #[test]
    fn test_convert_rockspec_without_optional_fields() {
        let temp = TempDir::new().unwrap();
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let manifest =
            convert_rockspec_to_manifest(&rockspec, temp.path(), "test-package").unwrap();
        assert_eq!(manifest.name, "test-package");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn test_convert_rockspec_with_dependencies() {
        let temp = TempDir::new().unwrap();
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec!["luasocket >= 3.0".to_string(), "penlight".to_string()],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let manifest =
            convert_rockspec_to_manifest(&rockspec, temp.path(), "test-package").unwrap();
        assert!(manifest.dependencies.contains_key("luasocket"));
        assert!(manifest.dependencies.contains_key("penlight"));
    }

    #[test]
    fn test_convert_rockspec_with_build_modules() {
        let temp = TempDir::new().unwrap();
        let mut modules = HashMap::new();
        modules.insert("socket".to_string(), "src/socket.lua".to_string());
        modules.insert("http".to_string(), "src/http.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let manifest =
            convert_rockspec_to_manifest(&rockspec, temp.path(), "test-package").unwrap();
        assert_eq!(manifest.name, "test-package");
        let manifest_path = temp
            .path()
            .join("lua_modules")
            .join(".depot")
            .join("packages")
            .join("test-package")
            .join("package.yaml");
        assert!(manifest_path.exists());
    }
}
