use lpm_core::{LpmError, LpmResult};
use serde::{Deserialize, Serialize};

/// Plugin registry entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Plugin name
    pub name: String,
    /// Latest version
    pub version: String,
    /// Description
    pub description: Option<String>,
    /// Author
    pub author: Option<String>,
    /// Homepage/repository
    pub homepage: Option<String>,
    /// Download URL
    pub download_url: Option<String>,
    /// Available versions
    pub versions: Vec<String>,
}

/// GitHub Release API response
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(skip)]
    _name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
    #[serde(skip)]
    _published_at: String,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(skip)]
    _size: u64,
}

/// Plugin registry (for discovering and installing plugins)
pub struct PluginRegistry;

impl PluginRegistry {
    /// Search for plugins in registry
    ///
    /// Searches crates.io for packages matching "lpm-*"
    pub async fn search(query: &str) -> LpmResult<Vec<RegistryEntry>> {
        let client = reqwest::Client::new();

        // Search crates.io for lpm-* packages
        let url = format!(
            "https://crates.io/api/v1/crates?q={}&per_page=20",
            urlencoding::encode(query)
        );

        #[derive(Deserialize)]
        struct CratesResponse {
            crates: Vec<CrateInfo>,
        }

        #[derive(Deserialize)]
        struct CrateInfo {
            name: String,
            description: Option<String>,
            repository: Option<String>,
            #[serde(rename = "max_version")]
            version: String,
        }

        let response = client
            .get(&url)
            .header("User-Agent", "lpm/0.1.0")
            .send()
            .await
            .map_err(LpmError::Http)?;

        if !response.status().is_success() {
            return Err(LpmError::Package(format!(
                "Registry search failed with status: {}",
                response.status()
            )));
        }

        let crates_resp: CratesResponse = response
            .json()
            .await
            .map_err(|e| LpmError::Package(format!("Failed to parse registry response: {}", e)))?;

        let mut results = Vec::new();
        for crate_info in crates_resp.crates {
            // Only include packages that start with "lpm-" prefix.
            if let Some(plugin_name) = crate_info.name.strip_prefix("lpm-") {
                let plugin_name = plugin_name.to_string();
                let version = crate_info.version.clone();
                results.push(RegistryEntry {
                    name: plugin_name,
                    version: version.clone(),
                    description: crate_info.description,
                    author: None,
                    homepage: crate_info.repository,
                    download_url: None,
                    versions: vec![version],
                });
            }
        }

        Ok(results)
    }

    /// Get plugin information from registry
    ///
    /// Tries to find plugin on GitHub by checking common repository patterns
    pub async fn get_plugin(name: &str) -> LpmResult<Option<RegistryEntry>> {
        // Try to find GitHub repository for lpm-{name}.
        // Common patterns:
        // - github.com/{user}/lpm-{name}
        // - github.com/{user}/{name}
        // - github.com/lpm-org/lpm-{name}

        // Attempt to fetch from GitHub releases using common repository patterns.
        let repo_patterns = vec![
            format!("lpm-org/lpm-{}", name),
            format!("{}/lpm-{}", name, name), // Username same as plugin name.
        ];

        for repo in repo_patterns {
            if let Ok(Some(entry)) = Self::get_plugin_from_github(&repo, name).await {
                return Ok(Some(entry));
            }
        }

        // Fallback: search crates.io if GitHub lookup fails.
        let search_results = Self::search(name).await?;
        Ok(search_results.into_iter().find(|e| e.name == name))
    }

    /// Get plugin from GitHub releases
    async fn get_plugin_from_github(
        repo: &str,
        plugin_name: &str,
    ) -> LpmResult<Option<RegistryEntry>> {
        let client = reqwest::Client::new();
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

        let response = client
            .get(&url)
            .header("User-Agent", "lpm/0.1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await;

        let release: GitHubRelease = match response {
            Ok(resp) if resp.status().is_success() => resp.json().await.map_err(|e| {
                LpmError::Package(format!("Failed to parse GitHub response: {}", e))
            })?,
            _ => return Ok(None),
        };

        // Find binary asset matching current platform (look for platform-specific binaries).
        let binary_asset = release.assets.iter().find(|asset| {
            let name = asset.name.to_lowercase();
            name.contains("lpm-")
                && (name.ends_with(".exe")
                    || name.ends_with("x86_64")
                    || name.ends_with("aarch64")
                    || name.contains("linux")
                    || name.contains("macos")
                    || name.contains("darwin")
                    || name.contains("windows"))
        });

        Ok(Some(RegistryEntry {
            name: plugin_name.to_string(),
            version: release
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&release.tag_name)
                .to_string(),
            description: release.body.clone(),
            author: None,
            homepage: Some(format!("https://github.com/{}", repo)),
            download_url: binary_asset.map(|a| a.browser_download_url.clone()),
            versions: vec![release.tag_name.trim_start_matches('v').to_string()],
        }))
    }

    /// Get latest version of a plugin
    pub async fn get_latest_version(name: &str) -> LpmResult<Option<String>> {
        if let Ok(Some(entry)) = Self::get_plugin(name).await {
            return Ok(Some(entry.version));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_entry_creation() {
        let entry = RegistryEntry {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test plugin".to_string()),
            author: Some("Test Author".to_string()),
            homepage: Some("https://example.com".to_string()),
            download_url: Some("https://example.com/download".to_string()),
            versions: vec!["1.0.0".to_string(), "0.9.0".to_string()],
        };

        assert_eq!(entry.name, "test-plugin");
        assert_eq!(entry.version, "1.0.0");
        assert_eq!(entry.versions.len(), 2);
        assert!(entry.description.is_some());
    }

    #[test]
    fn test_registry_entry_minimal() {
        let entry = RegistryEntry {
            name: "minimal".to_string(),
            version: "2.0.0".to_string(),
            description: None,
            author: None,
            homepage: None,
            download_url: None,
            versions: vec![],
        };

        assert_eq!(entry.name, "minimal");
        assert!(entry.description.is_none());
        assert!(entry.versions.is_empty());
    }

    #[test]
    fn test_registry_struct_exists() {
        // Ensure the registry struct can be constructed
        let _registry = PluginRegistry;
    }

    #[test]
    fn test_github_asset_deserialization() {
        let json = r#"{
            "name": "lpm-plugin-macos",
            "browser_download_url": "https://github.com/example/releases/download/v1.0.0/lpm-plugin-macos",
            "size": 1234567
        }"#;

        let asset: Result<GitHubAsset, _> = serde_json::from_str(json);
        assert!(asset.is_ok());
        let asset = asset.unwrap();
        assert_eq!(asset.name, "lpm-plugin-macos");
        assert!(asset.browser_download_url.contains("github.com"));
    }

    #[test]
    fn test_registry_entry_serialization() {
        let entry = RegistryEntry {
            name: "serialize-test".to_string(),
            version: "1.2.3".to_string(),
            description: Some("Test".to_string()),
            author: None,
            homepage: None,
            download_url: None,
            versions: vec!["1.2.3".to_string()],
        };

        let json = serde_json::to_string(&entry);
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("serialize-test"));
        assert!(json_str.contains("1.2.3"));
    }
}
