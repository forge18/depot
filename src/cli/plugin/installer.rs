use crate::cli::plugin::metadata::PluginMetadata;
use lpm::core::path::lpm_home;
use lpm::core::{LpmError, LpmResult};
use reqwest;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Install or update a plugin
pub struct PluginInstaller;

impl PluginInstaller {
    /// Create plugin metadata from entry data
    fn create_metadata(
        plugin_name: &str,
        target_version: &str,
        description: Option<String>,
        author: Option<String>,
        homepage: Option<String>,
    ) -> PluginMetadata {
        PluginMetadata {
            name: plugin_name.to_string(),
            version: target_version.to_string(),
            description,
            author,
            homepage,
            dependencies: vec![],
            min_lpm_version: None,
        }
    }

    /// Get the plugin installation path
    fn get_plugin_path(plugin_name: &str) -> LpmResult<PathBuf> {
        let lpm_home = lpm_home()?;
        let bin_dir = lpm_home.join("bin");
        Ok(bin_dir.join(format!("lpm-{}", plugin_name)))
    }

    /// Download and install a plugin
    pub async fn install(plugin_name: &str, version: Option<&str>) -> LpmResult<()> {
        use crate::cli::plugin::registry::PluginRegistry;

        println!("Installing plugin: {}", plugin_name);

        // Get plugin info from registry
        let entry = PluginRegistry::get_plugin(plugin_name)
            .await?
            .ok_or_else(|| {
                LpmError::Package(format!("Plugin '{}' not found in registry", plugin_name))
            })?;

        let target_version = version.unwrap_or(&entry.version);
        println!("  Version: {}", target_version);

        // Get download URL
        let download_url = entry.download_url.ok_or_else(|| {
            LpmError::Package(format!(
                "No download URL available for plugin '{}'",
                plugin_name
            ))
        })?;

        println!("  Downloading from: {}", download_url);

        // Download the binary
        let client = reqwest::Client::new();
        let response = client
            .get(&download_url)
            .header("User-Agent", "lpm/0.1.0")
            .send()
            .await
            .map_err(LpmError::Http)?;

        if !response.status().is_success() {
            return Err(LpmError::Package(format!(
                "Download failed with status: {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| LpmError::Package(format!("Failed to read download: {}", e)))?;

        // Determine installation path
        let lpm_home = lpm_home()?;
        let bin_dir = lpm_home.join("bin");
        fs::create_dir_all(&bin_dir)?;

        let plugin_path = bin_dir.join(format!("lpm-{}", plugin_name));

        // Write binary
        let mut file = fs::File::create(&plugin_path)?;
        file.write_all(&bytes)?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&plugin_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&plugin_path, perms)?;
        }

        // Save metadata
        let metadata = Self::create_metadata(
            plugin_name,
            target_version,
            entry.description,
            entry.author,
            entry.homepage,
        );

        let metadata_path = PluginMetadata::metadata_path(plugin_name)?;
        metadata.save(&metadata_path)?;

        println!("✓ Installed {}@{}", plugin_name, target_version);
        println!("  Location: {}", plugin_path.display());

        Ok(())
    }

    /// Update an existing plugin to latest version
    pub async fn update(plugin_name: &str) -> LpmResult<()> {
        use crate::cli::plugin::registry::PluginRegistry;
        use crate::cli::plugin::PluginInfo;

        // Get current version
        let current_info = PluginInfo::from_installed(plugin_name)?.ok_or_else(|| {
            LpmError::Package(format!("Plugin '{}' is not installed", plugin_name))
        })?;

        let current_version = current_info
            .installed_version
            .as_ref()
            .unwrap_or(&current_info.metadata.version);

        // Get latest version
        let latest_version = PluginRegistry::get_latest_version(plugin_name)
            .await?
            .ok_or_else(|| {
                LpmError::Package(format!(
                    "Could not determine latest version for '{}'",
                    plugin_name
                ))
            })?;

        if current_version == &latest_version {
            println!(
                "{} is already up to date (v{})",
                plugin_name, current_version
            );
            return Ok(());
        }

        println!(
            "Updating {} from v{} to v{}...",
            plugin_name, current_version, latest_version
        );

        // Install latest version
        Self::install(plugin_name, Some(&latest_version)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_metadata_with_all_fields() {
        let metadata = PluginInstaller::create_metadata(
            "test-plugin",
            "1.0.0",
            Some("Test description".to_string()),
            Some("Test Author".to_string()),
            Some("https://example.com".to_string()),
        );

        assert_eq!(metadata.name, "test-plugin");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.description, Some("Test description".to_string()));
        assert_eq!(metadata.author, Some("Test Author".to_string()));
        assert_eq!(metadata.homepage, Some("https://example.com".to_string()));
        assert!(metadata.dependencies.is_empty());
        assert!(metadata.min_lpm_version.is_none());
    }

    #[test]
    fn test_create_metadata_with_minimal_fields() {
        let metadata = PluginInstaller::create_metadata(
            "minimal-plugin",
            "2.5.1",
            None,
            None,
            None,
        );

        assert_eq!(metadata.name, "minimal-plugin");
        assert_eq!(metadata.version, "2.5.1");
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.author, None);
        assert_eq!(metadata.homepage, None);
    }

    #[test]
    fn test_get_plugin_path_format() {
        // This will fail if lpm_home() fails, but tests the path construction
        if let Ok(path) = PluginInstaller::get_plugin_path("test-plugin") {
            let path_str = path.to_string_lossy();
            assert!(path_str.contains("lpm-test-plugin"));
            assert!(path_str.contains("bin"));
        }
    }

    #[test]
    fn test_plugin_installer_exists() {
        // Test that the struct exists and can be referenced
        let _installer = PluginInstaller;
    }
}
