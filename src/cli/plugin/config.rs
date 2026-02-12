use depot_core::{DepotError, DepotResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Plugin configuration stored per-plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin name
    pub plugin_name: String,
    /// Configuration key-value pairs
    pub settings: std::collections::HashMap<String, serde_yaml::Value>,
}

impl PluginConfig {
    /// Load plugin configuration
    pub fn load(plugin_name: &str) -> DepotResult<Self> {
        let config_path = Self::config_path(plugin_name)?;

        if !config_path.exists() {
            // Return default config
            return Ok(PluginConfig {
                plugin_name: plugin_name.to_string(),
                settings: std::collections::HashMap::new(),
            });
        }

        let content = fs::read_to_string(&config_path)?;
        let config: PluginConfig = serde_yaml::from_str(&content)
            .map_err(|e| DepotError::Config(format!("Invalid plugin config: {}", e)))?;

        Ok(config)
    }

    /// Save plugin configuration
    pub fn save(&self) -> DepotResult<()> {
        let config_path = Self::config_path(&self.plugin_name)?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_yaml::to_string(self)
            .map_err(|e| DepotError::Config(format!("Failed to serialize config: {}", e)))?;
        fs::write(&config_path, content)?;

        Ok(())
    }

    /// Get configuration path for a plugin
    pub fn config_path(plugin_name: &str) -> DepotResult<PathBuf> {
        let depot_home = depot_core::core::path::depot_home()?;
        Ok(depot_home
            .join("plugins")
            .join(format!("{}.config.yaml", plugin_name)))
    }

    /// Get a setting value
    pub fn get<T>(&self, key: &str) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.settings
            .get(key)
            .and_then(|v| serde_yaml::from_value(v.clone()).ok())
    }

    /// Set a setting value
    pub fn set<T>(&mut self, key: String, value: T) -> DepotResult<()>
    where
        T: Serialize,
    {
        let yaml_value = serde_yaml::to_value(value)
            .map_err(|e| DepotError::Config(format!("Failed to serialize value: {}", e)))?;
        self.settings.insert(key, yaml_value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_plugin_config_load_nonexistent() {
        let _temp = TempDir::new().unwrap();

        let config = PluginConfig::load("test-plugin");
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.plugin_name, "test-plugin");
        assert!(config.settings.is_empty());
    }

    #[test]
    fn test_plugin_config_save_and_load() {
        let temp = TempDir::new().unwrap();
        let original_home = env::var("Depot_HOME").ok();
        env::set_var("Depot_HOME", temp.path());

        let mut config = PluginConfig {
            plugin_name: "save-test".to_string(),
            settings: std::collections::HashMap::new(),
        };

        config
            .set("key1".to_string(), "value1".to_string())
            .unwrap();
        config.set("key2".to_string(), 42).unwrap();

        let save_result = config.save();
        assert!(save_result.is_ok());

        let loaded = PluginConfig::load("save-test").unwrap();
        assert_eq!(loaded.plugin_name, "save-test");
        assert_eq!(loaded.get::<String>("key1"), Some("value1".to_string()));
        assert_eq!(loaded.get::<i32>("key2"), Some(42));

        // Restore original Depot_HOME
        if let Some(home) = original_home {
            env::set_var("Depot_HOME", home);
        } else {
            env::remove_var("Depot_HOME");
        }
    }

    #[test]
    fn test_plugin_config_get_nonexistent_key() {
        let config = PluginConfig {
            plugin_name: "test".to_string(),
            settings: std::collections::HashMap::new(),
        };

        let value: Option<String> = config.get("nonexistent");
        assert_eq!(value, None);
    }

    #[test]
    fn test_plugin_config_set() {
        let mut config = PluginConfig {
            plugin_name: "test".to_string(),
            settings: std::collections::HashMap::new(),
        };

        let result = config.set("test_key".to_string(), "test_value".to_string());
        assert!(result.is_ok());
        assert!(config.settings.contains_key("test_key"));
    }

    #[test]
    fn test_plugin_config_path() {
        let path = PluginConfig::config_path("my-plugin");
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains("my-plugin.config.yaml"));
    }
}
