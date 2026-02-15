use crate::core::{DepotError, DepotResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default = "default_lua_version")]
    pub lua_version: String,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub dev_dependencies: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub scripts: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub build: Option<BuildConfig>,
    #[serde(default)]
    pub binary_urls: std::collections::HashMap<String, String>, // target -> URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_strategy: Option<String>,
}

fn default_lua_version() -> String {
    "5.4".to_string()
}

/// Dependency specification for a package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencySpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(rename = "type")]
    pub build_type: String,
    #[serde(default)]
    pub manifest: Option<String>,
    #[serde(default)]
    pub modules: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub profile: Option<String>,
}

impl PackageManifest {
    /// Load package.yaml from a directory
    pub fn load(dir: &Path) -> DepotResult<Self> {
        let path = dir.join("package.yaml");
        if !path.exists() {
            return Err(DepotError::Package(format!(
                "package.yaml not found in {}",
                dir.display()
            )));
        }

        let content = fs::read_to_string(&path)?;
        let manifest: PackageManifest = serde_yaml::from_str(&content)
            .map_err(|e| DepotError::Package(format!("Failed to parse package.yaml: {}", e)))?;

        // Validate manifest (basic validation)
        manifest.validate()?;

        // Note: Additional schema validation (ManifestValidator) is not available in depot-core
        // Plugins or main depot crate can add their own validation if needed

        Ok(manifest)
    }

    /// Validate the manifest
    pub fn validate(&self) -> DepotResult<()> {
        // Validate name
        if self.name.is_empty() {
            return Err(DepotError::Package(
                "Package name cannot be empty".to_string(),
            ));
        }

        // Validate version format (basic SemVer check)
        if self.version.is_empty() {
            return Err(DepotError::Package(
                "Package version cannot be empty".to_string(),
            ));
        }

        // Validate lua_version
        if self.lua_version.is_empty() {
            return Err(DepotError::Package(
                "lua_version cannot be empty".to_string(),
            ));
        }

        // Validate dependencies don't have empty names or versions
        for (name, version) in &self.dependencies {
            if name.is_empty() {
                return Err(DepotError::Package(
                    "Dependency name cannot be empty".to_string(),
                ));
            }
            if version.is_empty() {
                return Err(DepotError::Package(format!(
                    "Dependency '{}' version cannot be empty",
                    name
                )));
            }
        }

        for (name, version) in &self.dev_dependencies {
            if name.is_empty() {
                return Err(DepotError::Package(
                    "Dev dependency name cannot be empty".to_string(),
                ));
            }
            if version.is_empty() {
                return Err(DepotError::Package(format!(
                    "Dev dependency '{}' version cannot be empty",
                    name
                )));
            }
        }

        Ok(())
    }

    /// Save package.yaml to a directory
    pub fn save(&self, dir: &Path) -> DepotResult<()> {
        let path = dir.join("package.yaml");
        let content = serde_yaml::to_string(self)
            .map_err(|e| DepotError::Package(format!("Failed to serialize package.yaml: {}", e)))?;

        fs::write(&path, content)?;
        Ok(())
    }

    /// Create a default manifest
    pub fn default(name: String) -> Self {
        Self {
            name,
            version: "1.0.0".to_string(),
            description: None,
            homepage: None,
            license: None,
            lua_version: "5.4".to_string(),
            dependencies: std::collections::HashMap::new(),
            dev_dependencies: std::collections::HashMap::new(),
            scripts: std::collections::HashMap::new(),
            build: None,
            binary_urls: std::collections::HashMap::new(),
            resolution_strategy: None,
        }
    }

    /// Get production dependencies (excluding dev_dependencies)
    ///
    /// This is useful for production builds where dev dependencies should be excluded.
    pub fn production_dependencies(&self) -> &std::collections::HashMap<String, String> {
        &self.dependencies
    }

    /// Check if a package is a dev dependency
    pub fn is_dev_dependency(&self, package_name: &str) -> bool {
        self.dev_dependencies.contains_key(package_name)
    }

    /// Get all dependencies (regular + dev) as a combined map
    ///
    /// Note: This does not handle conflicts - a package should not be in both
    /// dependencies and dev_dependencies. Use ConflictChecker to validate.
    pub fn all_dependencies(&self) -> std::collections::HashMap<String, String> {
        let mut all = self.dependencies.clone();
        for (name, version) in &self.dev_dependencies {
            // Only add if not already in regular dependencies
            if !all.contains_key(name) {
                all.insert(name.clone(), version.clone());
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_manifest() {
        let temp = TempDir::new().unwrap();
        let manifest_content = r#"
name: test-package
version: 1.0.0
lua_version: "5.4"
dependencies:
  luasocket: "3.0.0"
"#;
        fs::write(temp.path().join("package.yaml"), manifest_content).unwrap();

        let manifest = PackageManifest::load(temp.path()).unwrap();
        assert_eq!(manifest.name, "test-package");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.lua_version, "5.4");
        assert_eq!(manifest.dependencies.len(), 1);
    }

    #[test]
    fn test_save_manifest() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        manifest.save(temp.path()).unwrap();

        assert!(temp.path().join("package.yaml").exists());
    }
}
