use depot_core::{DepotError, DepotResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Plugin metadata stored in plugin directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name (without depot- prefix)
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: Option<String>,
    /// Plugin author
    pub author: Option<String>,
    /// Plugin homepage/repository
    pub homepage: Option<String>,
    /// Plugin dependencies (other plugins this plugin requires)
    pub dependencies: Vec<PluginDependency>,
    /// Minimum Depot version required
    pub min_depot_version: Option<String>,
}

/// Plugin dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    /// Plugin name (without depot- prefix)
    pub name: String,
    /// Version constraint (e.g., "^1.0.0", ">=0.5.0")
    pub version: Option<String>,
}

impl PluginMetadata {
    /// Load plugin metadata from a file
    pub fn load(metadata_path: &Path) -> DepotResult<Self> {
        if !metadata_path.exists() {
            return Err(DepotError::Package(format!(
                "Plugin metadata not found: {}",
                metadata_path.display()
            )));
        }

        let content = fs::read_to_string(metadata_path)?;
        let metadata: PluginMetadata = serde_yaml::from_str(&content)
            .map_err(|e| DepotError::Package(format!("Invalid plugin metadata: {}", e)))?;

        Ok(metadata)
    }

    /// Save plugin metadata to a file
    pub fn save(&self, metadata_path: &Path) -> DepotResult<()> {
        if let Some(parent) = metadata_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_yaml::to_string(self)
            .map_err(|e| DepotError::Package(format!("Failed to serialize metadata: {}", e)))?;
        fs::write(metadata_path, content)?;

        Ok(())
    }

    /// Get metadata file path for a plugin
    pub fn metadata_path(plugin_name: &str) -> DepotResult<PathBuf> {
        let lpm_home = depot_core::core::path::depot_home()?;
        Ok(lpm_home
            .join("plugins")
            .join(format!("{}.yaml", plugin_name)))
    }
}

/// Plugin information (metadata + installation info)
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin metadata
    pub metadata: PluginMetadata,
    /// Path to plugin executable
    pub executable_path: PathBuf,
    /// Installed version (may differ from metadata version)
    pub installed_version: Option<String>,
}

impl PluginInfo {
    /// Load plugin info from installed plugin
    pub fn from_installed(plugin_name: &str) -> DepotResult<Option<Self>> {
        use crate::cli::plugin::find_plugin;

        if let Some(executable_path) = find_plugin(plugin_name) {
            // Try to load metadata
            let metadata_path = PluginMetadata::metadata_path(plugin_name)?;
            let metadata = if metadata_path.exists() {
                PluginMetadata::load(&metadata_path)?
            } else {
                // Create default metadata from executable
                PluginMetadata {
                    name: plugin_name.to_string(),
                    version: "unknown".to_string(),
                    description: None,
                    author: None,
                    homepage: None,
                    dependencies: vec![],
                    min_depot_version: None,
                }
            };

            // Try to get version from executable (if it supports --version)
            let installed_version = Self::get_executable_version(&executable_path)
                .ok()
                .flatten();

            Ok(Some(PluginInfo {
                metadata,
                executable_path,
                installed_version,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get version from plugin executable
    fn get_executable_version(executable_path: &Path) -> DepotResult<Option<String>> {
        use std::process::Command;

        let output = Command::new(executable_path).arg("--version").output();

        match output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Ok(Some(version))
            }
            _ => Ok(None),
        }
    }
}
