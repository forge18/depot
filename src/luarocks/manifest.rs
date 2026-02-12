use crate::core::{DepotError, DepotResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// LuaRocks manifest structure
///
/// The manifest is a Lua table that maps package names to version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Repository name (e.g., "luarocks")
    pub repository: String,
    /// Packages in the manifest
    pub packages: HashMap<String, Vec<PackageVersion>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersion {
    /// Version string (e.g., "3.0-1")
    pub version: String,
    /// Rockspec URL
    pub rockspec_url: String,
    /// Archive URL (source package)
    pub archive_url: Option<String>,
}

/// JSON structure returned by LuaRocks API
#[derive(Debug, Deserialize)]
struct ManifestJson {
    repository: HashMap<String, HashMap<String, Vec<ArchInfo>>>,
}

#[derive(Debug, Deserialize)]
struct ArchInfo {
    arch: String,
}

impl Manifest {
    /// Parse manifest from JSON format (LuaRocks API)
    ///
    /// The LuaRocks manifest API returns JSON with structure:
    /// {
    ///   "repository": {
    ///     "package_name": {
    ///       "version": [{"arch": "rockspec"}, {"arch": "src"}]
    ///     }
    ///   }
    /// }
    pub fn parse_json(content: &str) -> DepotResult<Self> {
        let json: ManifestJson = serde_json::from_str(content)
            .map_err(|e| DepotError::Package(format!("Failed to parse manifest JSON: {}", e)))?;

        // The JSON structure is: {"repository": {"package_name": {"version": [...]}}}
        // So json.repository is already the packages map
        let repository_name = "luarocks".to_string(); // Default repository name

        // Convert JSON structure to Manifest structure
        let mut packages = HashMap::new();
        for (package_name, versions_map) in &json.repository {
            let mut package_versions = Vec::new();

            for (version_str, arch_infos) in versions_map {
                // Check if this version has a rockspec
                let has_rockspec = arch_infos.iter().any(|ai| ai.arch == "rockspec");

                if has_rockspec {
                    // Construct rockspec URL
                    let rockspec_url = format!(
                        "https://luarocks.org/manifests/{}/{}-{}.rockspec",
                        repository_name, package_name, version_str
                    );

                    // Archive URL will be extracted from rockspec when downloaded
                    package_versions.push(PackageVersion {
                        version: version_str.clone(),
                        rockspec_url,
                        archive_url: None,
                    });
                }
            }

            if !package_versions.is_empty() {
                packages.insert(package_name.clone(), package_versions);
            }
        }

        Ok(Manifest {
            repository: repository_name,
            packages,
        })
    }

    /// Parse manifest from Lua table format (legacy, not implemented)
    pub fn parse_lua(_content: &str) -> DepotResult<Self> {
        Err(DepotError::NotImplemented(
            "Lua manifest parsing not implemented. Use JSON format instead.".to_string(),
        ))
    }

    /// Get all versions of a package
    pub fn get_package_versions(&self, package_name: &str) -> Option<&Vec<PackageVersion>> {
        self.packages.get(package_name)
    }

    /// Get latest version of a package
    pub fn get_latest_version(&self, package_name: &str) -> Option<&PackageVersion> {
        self.get_package_versions(package_name)?
            .iter()
            .max_by_key(|pv| &pv.version)
    }

    /// Get all version strings for a package
    pub fn get_package_version_strings(&self, package_name: &str) -> Vec<String> {
        self.get_package_versions(package_name)
            .map(|versions| versions.iter().map(|pv| pv.version.clone()).collect())
            .unwrap_or_default()
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            repository: "luarocks".to_string(),
            packages: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_default() {
        let manifest = Manifest::default();
        assert_eq!(manifest.repository, "luarocks");
        assert!(manifest.packages.is_empty());
    }

    #[test]
    fn test_manifest_parse_json() {
        let json = r#"{
            "repository": {
                "test-package": {
                    "1.0.0": [{"arch": "rockspec"}],
                    "2.0.0": [{"arch": "rockspec"}]
                }
            }
        }"#;

        let manifest = Manifest::parse_json(json).unwrap();
        assert_eq!(manifest.repository, "luarocks");
        assert!(manifest.packages.contains_key("test-package"));
        let versions = manifest.get_package_versions("test-package").unwrap();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_manifest_parse_json_invalid() {
        let json = "invalid json";
        let result = Manifest::parse_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_manifest_get_package_versions() {
        let mut manifest = Manifest::default();
        let versions = vec![
            PackageVersion {
                version: "1.0.0".to_string(),
                rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
                archive_url: Some("https://example.com/test-1.0.0.tar.gz".to_string()),
            },
            PackageVersion {
                version: "2.0.0".to_string(),
                rockspec_url: "https://example.com/test-2.0.0.rockspec".to_string(),
                archive_url: None,
            },
        ];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let versions = manifest.get_package_versions("test-package");
        assert!(versions.is_some());
        assert_eq!(versions.unwrap().len(), 2);

        let none = manifest.get_package_versions("nonexistent");
        assert!(none.is_none());
    }

    #[test]
    fn test_manifest_get_latest_version() {
        let mut manifest = Manifest::default();
        let versions = vec![
            PackageVersion {
                version: "1.0.0".to_string(),
                rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
                archive_url: None,
            },
            PackageVersion {
                version: "2.0.0".to_string(),
                rockspec_url: "https://example.com/test-2.0.0.rockspec".to_string(),
                archive_url: None,
            },
        ];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let latest = manifest.get_latest_version("test-package");
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().version, "2.0.0");
    }

    #[test]
    fn test_manifest_get_package_version_strings() {
        let mut manifest = Manifest::default();
        let versions = vec![
            PackageVersion {
                version: "1.0.0".to_string(),
                rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
                archive_url: None,
            },
            PackageVersion {
                version: "2.0.0".to_string(),
                rockspec_url: "https://example.com/test-2.0.0.rockspec".to_string(),
                archive_url: None,
            },
        ];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let version_strings = manifest.get_package_version_strings("test-package");
        assert_eq!(version_strings.len(), 2);
        assert!(version_strings.contains(&"1.0.0".to_string()));
        assert!(version_strings.contains(&"2.0.0".to_string()));

        let empty = manifest.get_package_version_strings("nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_manifest_parse_lua_not_implemented() {
        let result = Manifest::parse_lua("some lua content");
        assert!(result.is_err());
        match result {
            Err(DepotError::NotImplemented(_)) => {}
            _ => panic!("Expected NotImplemented error"),
        }
    }

    #[test]
    fn test_manifest_parse_json_without_rockspec() {
        let json = r#"{
            "repository": {
                "test-package": {
                    "1.0.0": [{"arch": "src"}]
                }
            }
        }"#;

        let manifest = Manifest::parse_json(json).unwrap();
        // Package without rockspec should not be included
        assert!(!manifest.packages.contains_key("test-package"));
    }

    #[test]
    fn test_manifest_parse_json_invalid_format() {
        let json = r#"{"invalid": "json"}"#;
        let result = Manifest::parse_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_manifest_parse_json_empty() {
        let json = r#"{"repository": {}}"#;
        let manifest = Manifest::parse_json(json).unwrap();
        assert!(manifest.packages.is_empty());
    }

    #[test]
    fn test_manifest_get_latest_version_nonexistent() {
        let manifest = Manifest::default();
        let latest = manifest.get_latest_version("nonexistent");
        assert!(latest.is_none());
    }

    #[test]
    fn test_manifest_get_package_versions_nonexistent() {
        let manifest = Manifest::default();
        let versions = manifest.get_package_versions("nonexistent");
        assert!(versions.is_none());
    }

    #[test]
    fn test_manifest_get_latest_version_single() {
        let mut manifest = Manifest::default();
        let versions = vec![PackageVersion {
            version: "1.0.0".to_string(),
            rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
            archive_url: None,
        }];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let latest = manifest.get_latest_version("test-package");
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().version, "1.0.0");
    }

    #[test]
    fn test_manifest_parse_json_multiple_packages() {
        let json = r#"{
            "repository": {
                "package1": {
                    "1.0.0": [{"arch": "rockspec"}]
                },
                "package2": {
                    "2.0.0": [{"arch": "rockspec"}]
                }
            }
        }"#;

        let manifest = Manifest::parse_json(json).unwrap();
        assert_eq!(manifest.packages.len(), 2);
        assert!(manifest.packages.contains_key("package1"));
        assert!(manifest.packages.contains_key("package2"));
    }
}
