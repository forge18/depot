use crate::core::path::{config_file, ensure_dir};
use crate::core::{DepotError, DepotResult};
use crate::di::ConfigProvider;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// LuaRocks manifest URL
    #[serde(default = "default_luarocks_manifest_url")]
    pub luarocks_manifest_url: String,

    /// Cache directory (defaults to platform-specific cache directory)
    ///
    /// Default locations:
    /// - Windows: %LOCALAPPDATA%\depot\cache
    /// - Linux: ~/.cache/depot
    /// - macOS: ~/Library/Caches/depot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<String>,

    /// Whether to verify checksums on install
    #[serde(default = "default_true")]
    pub verify_checksums: bool,

    /// Whether to show diffs on update
    #[serde(default = "default_true")]
    pub show_diffs_on_update: bool,

    /// Default Lua binary source URL (defaults to dyne/luabinaries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lua_binary_source_url: Option<String>,

    /// Per-version Lua binary source URLs
    /// Example: { "5.4.8": "https://custom-source.com/binaries" }
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lua_binary_sources: Option<std::collections::HashMap<String, String>>,

    /// Resolution strategy for selecting package versions
    /// - "highest": Select the highest compatible version (default)
    /// - "lowest": Select the lowest compatible version
    #[serde(default = "default_resolution_strategy")]
    pub resolution_strategy: String,

    /// Checksum algorithm for package verification
    /// - "blake3": BLAKE3 (default, faster and more secure)
    /// - "sha256": SHA-256 (legacy, for backward compatibility)
    #[serde(default = "default_checksum_algorithm")]
    pub checksum_algorithm: String,

    /// Override for supported Lua versions (optional)
    /// If set, only these versions will be offered in interactive prompts
    /// Example: ["5.1", "5.3", "5.4"]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_lua_versions: Option<Vec<String>>,

    /// Enable strict conflict detection mode
    /// When enabled, performs additional checks:
    /// - Transitive dependency conflicts
    /// - Diamond dependency version mismatches
    /// - Constraint satisfiability verification
    /// - Phantom dependency warnings
    #[serde(default = "default_true")]
    pub strict_conflicts: bool,
}

fn default_checksum_algorithm() -> String {
    "blake3".to_string()
}

fn default_resolution_strategy() -> String {
    "highest".to_string()
}

fn default_luarocks_manifest_url() -> String {
    "https://luarocks.org/manifests/luarocks/manifest".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            luarocks_manifest_url: default_luarocks_manifest_url(),
            cache_dir: None,
            verify_checksums: true,
            show_diffs_on_update: true,
            lua_binary_source_url: None,
            lua_binary_sources: None,
            resolution_strategy: default_resolution_strategy(),
            checksum_algorithm: default_checksum_algorithm(),
            supported_lua_versions: None,
            strict_conflicts: true,
        }
    }
}

impl Config {
    /// Load config from platform-specific config directory, creating default if it doesn't exist
    ///
    /// Config locations:
    /// - Windows: %APPDATA%\depot\config.yaml
    /// - Linux: ~/.config/depot/config.yaml
    /// - macOS: ~/Library/Application Support/depot/config.yaml
    pub fn load() -> DepotResult<Self> {
        let config_path = config_file()?;

        if !config_path.exists() {
            // Create default config
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&config_path)?;
        let config: Config = serde_yaml::from_str(&content)
            .map_err(|e| DepotError::Config(format!("Failed to parse config: {}", e)))?;

        Ok(config)
    }

    /// Save config to platform-specific config directory
    ///
    /// Config locations:
    /// - Windows: %APPDATA%\depot\config.yaml
    /// - Linux: ~/.config/depot/config.yaml
    /// - macOS: ~/Library/Application Support/depot/config.yaml
    pub fn save(&self) -> DepotResult<()> {
        let config_path = config_file()?;
        let config_dir = config_path
            .parent()
            .ok_or_else(|| DepotError::Path("Invalid config path".to_string()))?;

        // Ensure config directory exists
        ensure_dir(config_dir)?;

        let content = serde_yaml::to_string(self)
            .map_err(|e| DepotError::Config(format!("Failed to serialize config: {}", e)))?;

        fs::write(&config_path, content)?;
        Ok(())
    }

    /// Get the cache directory path
    pub fn get_cache_dir(&self) -> DepotResult<std::path::PathBuf> {
        if let Some(ref dir) = self.cache_dir {
            Ok(std::path::PathBuf::from(dir))
        } else {
            crate::core::path::cache_dir()
        }
    }
}

// Implement ConfigProvider trait
impl ConfigProvider for Config {
    fn luarocks_manifest_url(&self) -> &str {
        &self.luarocks_manifest_url
    }

    fn cache_dir(&self) -> DepotResult<std::path::PathBuf> {
        self.get_cache_dir()
    }

    fn verify_checksums(&self) -> bool {
        self.verify_checksums
    }

    fn show_diffs_on_update(&self) -> bool {
        self.show_diffs_on_update
    }

    fn resolution_strategy(&self) -> &str {
        &self.resolution_strategy
    }

    fn checksum_algorithm(&self) -> &str {
        &self.checksum_algorithm
    }

    fn strict_conflicts(&self) -> bool {
        self.strict_conflicts
    }

    fn lua_binary_source_url(&self) -> Option<&str> {
        self.lua_binary_source_url.as_deref()
    }

    fn supported_lua_versions(&self) -> Option<&Vec<String>> {
        self.supported_lua_versions.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(
            config.luarocks_manifest_url,
            "https://luarocks.org/manifests/luarocks/manifest"
        );
        assert!(config.verify_checksums);
        assert!(config.show_diffs_on_update);
    }

    #[test]
    fn test_config_save_and_load() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("config.yaml");

        // Mock the config_file function for testing
        // In a real scenario, we'd use dependency injection
        let config = Config::default();
        let content = serde_yaml::to_string(&config).unwrap();
        std::fs::write(&config_path, content).unwrap();

        let loaded_content = std::fs::read_to_string(&config_path).unwrap();
        let loaded: Config = serde_yaml::from_str(&loaded_content).unwrap();

        assert_eq!(config.luarocks_manifest_url, loaded.luarocks_manifest_url);
    }

    #[test]
    fn test_config_get_cache_dir_default() {
        let config = Config::default();
        // Should use the default platform-specific cache dir
        let result = config.get_cache_dir();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_get_cache_dir_custom() {
        let config = Config {
            cache_dir: Some("/custom/cache/path".to_string()),
            ..Default::default()
        };
        let result = config.get_cache_dir().unwrap();
        assert_eq!(result.to_string_lossy(), "/custom/cache/path");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            cache_dir: Some("/tmp/cache".to_string()),
            verify_checksums: false,
            ..Default::default()
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("verify_checksums: false"));
        assert!(yaml.contains("cache_dir: /tmp/cache"));
    }

    #[test]
    fn test_config_deserialization() {
        let yaml = r#"
luarocks_manifest_url: https://custom.luarocks.org/manifest
verify_checksums: false
show_diffs_on_update: true
cache_dir: /custom/cache
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.luarocks_manifest_url,
            "https://custom.luarocks.org/manifest"
        );
        assert!(!config.verify_checksums);
        assert!(config.show_diffs_on_update);
        assert_eq!(config.cache_dir, Some("/custom/cache".to_string()));
    }

    #[test]
    fn test_config_deserialization_defaults() {
        let yaml = r#"
luarocks_manifest_url: https://custom.luarocks.org/manifest
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        // Should use default values for missing fields
        assert!(config.verify_checksums); // default is true
        assert!(config.show_diffs_on_update);
        assert!(config.cache_dir.is_none());
    }

    #[test]
    fn test_config_with_lua_binary_sources() {
        let mut sources = std::collections::HashMap::new();
        sources.insert(
            "5.4.8".to_string(),
            "https://custom-source.com/5.4.8".to_string(),
        );
        let config = Config {
            lua_binary_sources: Some(sources),
            ..Default::default()
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("5.4.8"));
    }

    #[test]
    fn test_config_with_lua_binary_source_url() {
        let config = Config {
            lua_binary_source_url: Some("https://custom-lua-binaries.com".to_string()),
            ..Default::default()
        };

        assert_eq!(
            config.lua_binary_source_url,
            Some("https://custom-lua-binaries.com".to_string())
        );
    }

    #[test]
    fn test_default_luarocks_manifest_url() {
        let url = default_luarocks_manifest_url();
        assert_eq!(url, "https://luarocks.org/manifests/luarocks/manifest");
    }

    #[test]
    fn test_default_true() {
        assert!(default_true());
    }

    #[test]
    fn test_config_with_resolution_strategy() {
        let config = Config {
            resolution_strategy: "lowest".to_string(),
            ..Default::default()
        };

        assert_eq!(config.resolution_strategy, "lowest");

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("resolution_strategy: lowest"));
    }

    #[test]
    fn test_config_default_resolution_strategy() {
        let config = Config::default();
        assert_eq!(config.resolution_strategy, "highest");
    }

    #[test]
    fn test_config_deserialization_with_resolution_strategy() {
        let yaml = r#"
luarocks_manifest_url: https://custom.luarocks.org/manifest
resolution_strategy: lowest
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.resolution_strategy, "lowest");
    }

    #[test]
    fn test_config_with_supported_lua_versions() {
        let config = Config {
            supported_lua_versions: Some(vec!["5.1".to_string(), "5.4".to_string()]),
            ..Default::default()
        };

        assert_eq!(
            config.supported_lua_versions,
            Some(vec!["5.1".to_string(), "5.4".to_string()])
        );

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("supported_lua_versions"));
        assert!(yaml.contains("5.1"));
        assert!(yaml.contains("5.4"));
    }

    #[test]
    fn test_config_deserialization_with_supported_lua_versions() {
        let yaml = r#"
luarocks_manifest_url: https://custom.luarocks.org/manifest
supported_lua_versions:
  - "5.1"
  - "5.3"
  - "5.4"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.supported_lua_versions,
            Some(vec![
                "5.1".to_string(),
                "5.3".to_string(),
                "5.4".to_string()
            ])
        );
    }

    #[test]
    fn test_config_default_no_supported_lua_versions() {
        let config = Config::default();
        assert_eq!(config.supported_lua_versions, None);
    }

    #[test]
    fn test_config_default_strict_conflicts() {
        let config = Config::default();
        assert!(config.strict_conflicts); // Now defaults to true
    }

    #[test]
    fn test_config_with_strict_conflicts() {
        let config = Config {
            strict_conflicts: true,
            ..Default::default()
        };
        assert!(config.strict_conflicts);

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("strict_conflicts: true"));
    }

    #[test]
    fn test_config_deserialization_with_strict_conflicts() {
        let yaml = r#"
luarocks_manifest_url: https://custom.luarocks.org/manifest
strict_conflicts: true
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.strict_conflicts);
    }

    #[test]
    fn test_config_provider_implementation() {
        let config = Config::default();
        let provider: &dyn ConfigProvider = &config;

        assert_eq!(
            provider.luarocks_manifest_url(),
            "https://luarocks.org/manifests/luarocks/manifest"
        );
        assert!(provider.verify_checksums());
        assert!(provider.show_diffs_on_update());
        assert_eq!(provider.resolution_strategy(), "highest");
        assert_eq!(provider.checksum_algorithm(), "blake3");
        assert!(provider.strict_conflicts());
        assert_eq!(provider.lua_binary_source_url(), None);
        assert_eq!(provider.supported_lua_versions(), None);
    }

    #[test]
    fn test_config_provider_cache_dir() {
        let config = Config {
            cache_dir: Some("/test/cache".to_string()),
            ..Default::default()
        };
        let provider: &dyn ConfigProvider = &config;

        let cache_dir = provider.cache_dir().unwrap();
        assert_eq!(cache_dir.to_string_lossy(), "/test/cache");
    }

    #[test]
    fn test_config_provider_with_custom_values() {
        let config = Config {
            luarocks_manifest_url: "https://custom.manifest.org".to_string(),
            verify_checksums: false,
            show_diffs_on_update: false,
            resolution_strategy: "lowest".to_string(),
            checksum_algorithm: "sha256".to_string(),
            strict_conflicts: false,
            lua_binary_source_url: Some("https://lua.org/binaries".to_string()),
            supported_lua_versions: Some(vec!["5.1".to_string()]),
            ..Default::default()
        };
        let provider: &dyn ConfigProvider = &config;

        assert_eq!(
            provider.luarocks_manifest_url(),
            "https://custom.manifest.org"
        );
        assert!(!provider.verify_checksums());
        assert!(!provider.show_diffs_on_update());
        assert_eq!(provider.resolution_strategy(), "lowest");
        assert_eq!(provider.checksum_algorithm(), "sha256");
        assert!(!provider.strict_conflicts());
        assert_eq!(
            provider.lua_binary_source_url(),
            Some("https://lua.org/binaries")
        );
        assert_eq!(provider.supported_lua_versions().unwrap().len(), 1);
    }

    #[test]
    fn test_config_default_checksum_algorithm() {
        let config = Config::default();
        assert_eq!(config.checksum_algorithm, "blake3");
        assert_eq!(default_checksum_algorithm(), "blake3");
    }

    #[test]
    fn test_config_with_sha256_checksum() {
        let config = Config {
            checksum_algorithm: "sha256".to_string(),
            ..Default::default()
        };

        assert_eq!(config.checksum_algorithm, "sha256");
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("checksum_algorithm: sha256"));
    }

    #[test]
    fn test_config_deserialization_with_checksum_algorithm() {
        let yaml = r#"
luarocks_manifest_url: https://custom.luarocks.org/manifest
checksum_algorithm: sha256
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.checksum_algorithm, "sha256");
    }

    #[test]
    fn test_default_resolution_strategy() {
        assert_eq!(default_resolution_strategy(), "highest");
    }

    #[test]
    fn test_config_serialization_with_all_fields() {
        let mut lua_sources = std::collections::HashMap::new();
        lua_sources.insert("5.4".to_string(), "https://example.com".to_string());

        let config = Config {
            luarocks_manifest_url: "https://custom.manifest.org".to_string(),
            cache_dir: Some("/cache".to_string()),
            verify_checksums: false,
            show_diffs_on_update: false,
            lua_binary_source_url: Some("https://binaries.org".to_string()),
            lua_binary_sources: Some(lua_sources),
            resolution_strategy: "lowest".to_string(),
            checksum_algorithm: "sha256".to_string(),
            supported_lua_versions: Some(vec!["5.1".to_string(), "5.4".to_string()]),
            strict_conflicts: false,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("luarocks_manifest_url: https://custom.manifest.org"));
        assert!(yaml.contains("cache_dir: /cache"));
        assert!(yaml.contains("verify_checksums: false"));
        assert!(yaml.contains("show_diffs_on_update: false"));
        assert!(yaml.contains("lua_binary_source_url: https://binaries.org"));
        assert!(yaml.contains("resolution_strategy: lowest"));
        assert!(yaml.contains("checksum_algorithm: sha256"));
        assert!(yaml.contains("strict_conflicts: false"));
    }
}
