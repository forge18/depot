use crate::core::DepotResult;
use crate::package::manifest::{BuildConfig, PackageManifest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed rockspec data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rockspec {
    pub package: String,
    pub version: String,
    pub source: RockspecSource,
    pub dependencies: Vec<String>,
    pub build: RockspecBuild,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub lua_version: Option<String>,
    #[serde(default)]
    pub binary_urls: HashMap<String, String>, // target -> URL
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RockspecSource {
    pub url: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RockspecBuild {
    #[serde(rename = "type")]
    pub build_type: String,
    #[serde(default)]
    pub modules: HashMap<String, String>,
    #[serde(default)]
    pub install: InstallTable,
}

/// Install table structure from rockspecs
/// Contains sections: bin, lua, lib, conf
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallTable {
    #[serde(default)]
    pub bin: HashMap<String, String>, // executable_name -> source_path
    #[serde(default)]
    pub lua: HashMap<String, String>, // module_name -> source_path
    #[serde(default)]
    pub lib: HashMap<String, String>, // library_name -> source_path
    #[serde(default)]
    pub conf: HashMap<String, String>, // config_name -> source_path
}

impl InstallTable {
    /// Check if the install table is empty (all sections empty)
    pub fn is_empty(&self) -> bool {
        self.bin.is_empty() && self.lua.is_empty() && self.lib.is_empty() && self.conf.is_empty()
    }
}

impl Rockspec {
    /// Parse rockspec from Lua content using regex-based parsing
    ///
    /// Rockspecs are simple data files, so we can parse them without
    /// embedding a Lua interpreter. This simplifies cross-compilation
    /// and removes the need for Lua version selection.
    pub fn parse_lua(content: &str) -> DepotResult<Self> {
        crate::luarocks::rockspec_parser::parse_rockspec(content)
    }

    /// Convert rockspec to PackageManifest format
    pub fn to_package_manifest(&self) -> PackageManifest {
        // Convert dependencies from LuaRocks format to Depot format
        let mut dependencies = HashMap::new();
        for dep in &self.dependencies {
            // Parse dependency like "lua >= 5.1" or "luasocket" or "luasocket ~> 3.0"
            // LuaRocks uses ~> for compatible versions, which we convert to ^
            let dep_str = dep.trim();
            if let Some((name, version)) = dep_str.split_once(' ') {
                let name = name.trim().to_string();
                let version = version.trim();
                // Convert LuaRocks ~> to SemVer ^
                let version = if version.starts_with("~>") {
                    version.replacen("~>", "^", 1)
                } else {
                    version.to_string()
                };
                dependencies.insert(name, version);
            } else {
                // No version specified, use wildcard
                dependencies.insert(dep_str.to_string(), "*".to_string());
            }
        }

        // Convert build config
        let build = if self.build.build_type == "builtin" || self.build.build_type == "none" {
            None
        } else {
            Some(BuildConfig {
                build_type: self.build.build_type.clone(),
                manifest: None,
                modules: self.build.modules.clone(),
                features: vec![],
                profile: None,
            })
        };

        PackageManifest {
            name: self.package.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            homepage: self.homepage.clone(),
            license: self.license.clone(),
            lua_version: self
                .lua_version
                .clone()
                .unwrap_or_else(|| ">=5.1".to_string()),
            dependencies,
            dev_dependencies: HashMap::new(),
            scripts: HashMap::new(),
            build,
            binary_urls: self.binary_urls.clone(),
            resolution_strategy: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_table_is_empty() {
        let table = InstallTable::default();
        assert!(table.is_empty());
    }

    #[test]
    fn test_install_table_not_empty_with_bin() {
        let mut table = InstallTable::default();
        table
            .bin
            .insert("myapp".to_string(), "bin/myapp".to_string());
        assert!(!table.is_empty());
    }

    #[test]
    fn test_install_table_not_empty_with_lua() {
        let mut table = InstallTable::default();
        table
            .lua
            .insert("mymodule".to_string(), "src/mymodule.lua".to_string());
        assert!(!table.is_empty());
    }

    #[test]
    fn test_rockspec_to_package_manifest() {
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec!["luasocket ~> 3.0".to_string()],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: Some("Test package".to_string()),
            homepage: Some("https://example.com".to_string()),
            license: Some("MIT".to_string()),
            lua_version: Some(">=5.1".to_string()),
            binary_urls: HashMap::new(),
        };

        let manifest = rockspec.to_package_manifest();
        assert_eq!(manifest.name, "test-package");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, Some("Test package".to_string()));
        assert!(manifest.dependencies.contains_key("luasocket"));
        // Should convert ~> to ^
        assert!(manifest
            .dependencies
            .get("luasocket")
            .unwrap()
            .starts_with("^"));
    }

    #[test]
    fn test_rockspec_to_package_manifest_without_version() {
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec!["luasocket".to_string()],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let manifest = rockspec.to_package_manifest();
        assert!(manifest.dependencies.contains_key("luasocket"));
        assert_eq!(manifest.dependencies.get("luasocket").unwrap(), "*");
    }

    #[test]
    fn test_rockspec_to_package_manifest_rust_build() {
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
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let manifest = rockspec.to_package_manifest();
        assert!(manifest.build.is_some());
        assert_eq!(manifest.build.as_ref().unwrap().build_type, "rust");
    }

    #[test]
    fn test_rockspec_to_package_manifest_builtin_no_build() {
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
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let manifest = rockspec.to_package_manifest();
        assert!(manifest.build.is_none());
    }
}
