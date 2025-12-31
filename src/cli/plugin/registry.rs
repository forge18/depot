use async_trait::async_trait;
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

/// HTTP client trait for dependency injection
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(&self, url: &str, headers: Vec<(&str, &str)>) -> LpmResult<HttpResponse>;
}

/// HTTP response abstraction
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    pub async fn json<T: serde::de::DeserializeOwned>(self) -> LpmResult<T> {
        serde_json::from_slice(&self.body)
            .map_err(|e| LpmError::Package(format!("Failed to parse JSON: {}", e)))
    }
}

/// Real HTTP client implementation using reqwest
pub struct ReqwestClient {
    client: reqwest::Client,
}

impl ReqwestClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestClient {
    async fn get(&self, url: &str, headers: Vec<(&str, &str)>) -> LpmResult<HttpResponse> {
        let mut request = self.client.get(url);
        for (key, value) in headers {
            request = request.header(key, value);
        }

        let response = request.send().await.map_err(LpmError::Http)?;
        let status = response.status().as_u16();
        let body = response
            .bytes()
            .await
            .map_err(|e| LpmError::Package(format!("Failed to read response body: {}", e)))?;

        Ok(HttpResponse {
            status,
            body: body.to_vec(),
        })
    }
}

/// Plugin registry (for discovering and installing plugins)
pub struct PluginRegistry<C: HttpClient = ReqwestClient> {
    client: C,
}

impl PluginRegistry<ReqwestClient> {
    pub fn new() -> Self {
        Self {
            client: ReqwestClient::new(),
        }
    }
}

impl Default for PluginRegistry<ReqwestClient> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: HttpClient> PluginRegistry<C> {
    pub fn _with_client(client: C) -> Self {
        Self { client }
    }
}

impl<C: HttpClient> PluginRegistry<C> {
    /// Search for plugins in registry
    ///
    /// Searches crates.io for packages matching "lpm-*"
    pub async fn search(&self, query: &str) -> LpmResult<Vec<RegistryEntry>> {
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

        let response = self
            .client
            .get(&url, vec![("User-Agent", "lpm/0.1.0")])
            .await?;

        if !response.is_success() {
            return Err(LpmError::Package(format!(
                "Registry search failed with status: {}",
                response.status
            )));
        }

        let crates_resp: CratesResponse = response.json().await?;

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
    pub async fn get_plugin(&self, name: &str) -> LpmResult<Option<RegistryEntry>> {
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
            if let Ok(Some(entry)) = self.get_plugin_from_github(&repo, name).await {
                return Ok(Some(entry));
            }
        }

        // Fallback: search crates.io if GitHub lookup fails.
        let search_results = self.search(name).await?;
        Ok(search_results.into_iter().find(|e| e.name == name))
    }

    /// Get plugin from GitHub releases
    async fn get_plugin_from_github(
        &self,
        repo: &str,
        plugin_name: &str,
    ) -> LpmResult<Option<RegistryEntry>> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

        let response = self
            .client
            .get(
                &url,
                vec![
                    ("User-Agent", "lpm/0.1.0"),
                    ("Accept", "application/vnd.github.v3+json"),
                ],
            )
            .await;

        let release: GitHubRelease = match response {
            Ok(resp) if resp.is_success() => resp.json().await?,
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
    pub async fn get_latest_version(&self, name: &str) -> LpmResult<Option<String>> {
        if let Ok(Some(entry)) = self.get_plugin(name).await {
            return Ok(Some(entry.version));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Mock HTTP client for testing
    struct MockHttpClient {
        responses: HashMap<String, (u16, Vec<u8>)>,
    }

    impl MockHttpClient {
        fn new() -> Self {
            Self {
                responses: HashMap::new(),
            }
        }

        fn with_response(mut self, url: &str, status: u16, body: Vec<u8>) -> Self {
            self.responses.insert(url.to_string(), (status, body));
            self
        }
    }

    #[async_trait]
    impl HttpClient for MockHttpClient {
        async fn get(&self, url: &str, _headers: Vec<(&str, &str)>) -> LpmResult<HttpResponse> {
            if let Some((status, body)) = self.responses.get(url) {
                Ok(HttpResponse {
                    status: *status,
                    body: body.clone(),
                })
            } else {
                // Return 404 for unknown URLs
                Ok(HttpResponse {
                    status: 404,
                    body: vec![],
                })
            }
        }
    }

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
        let _registry = PluginRegistry::new();
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

    #[tokio::test]
    async fn test_search_with_mock_client() {
        let mock_response = r#"{
            "crates": [
                {
                    "name": "lpm-test-plugin",
                    "description": "A test plugin",
                    "repository": "https://github.com/test/lpm-test-plugin",
                    "max_version": "1.0.0"
                },
                {
                    "name": "not-lpm-plugin",
                    "description": "Not an lpm plugin",
                    "repository": null,
                    "max_version": "1.0.0"
                },
                {
                    "name": "lpm-another",
                    "description": null,
                    "repository": null,
                    "max_version": "2.0.0"
                }
            ]
        }"#;

        let mock_client = MockHttpClient::new().with_response(
            "https://crates.io/api/v1/crates?q=test&per_page=20",
            200,
            mock_response.as_bytes().to_vec(),
        );

        let registry = PluginRegistry::_with_client(mock_client);
        let results = registry.search("test").await.unwrap();

        assert_eq!(results.len(), 2); // Only lpm-* packages
        assert_eq!(results[0].name, "test-plugin"); // Strip lpm- prefix
        assert_eq!(results[0].version, "1.0.0");
        assert_eq!(results[1].name, "another");
        assert_eq!(results[1].version, "2.0.0");
    }

    #[tokio::test]
    async fn test_search_with_error_response() {
        let mock_client = MockHttpClient::new().with_response(
            "https://crates.io/api/v1/crates?q=test&per_page=20",
            500,
            vec![],
        );

        let registry = PluginRegistry::_with_client(mock_client);
        let result = registry.search("test").await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Registry search failed"));
    }

    #[tokio::test]
    async fn test_get_plugin_from_github_success() {
        let mock_response = r#"{
            "tag_name": "v1.2.3",
            "name": "Release 1.2.3",
            "body": "Release notes",
            "published_at": "2024-01-01T00:00:00Z",
            "assets": [
                {
                    "name": "lpm-test-x86_64",
                    "browser_download_url": "https://github.com/test/releases/download/v1.2.3/lpm-test-x86_64",
                    "size": 1234567
                }
            ]
        }"#;

        let mock_client = MockHttpClient::new().with_response(
            "https://api.github.com/repos/lpm-org/lpm-test/releases/latest",
            200,
            mock_response.as_bytes().to_vec(),
        );

        let registry = PluginRegistry::_with_client(mock_client);
        let result = registry
            .get_plugin_from_github("lpm-org/lpm-test", "test")
            .await
            .unwrap();

        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.name, "test");
        assert_eq!(entry.version, "1.2.3"); // v prefix stripped
        assert_eq!(entry.description, Some("Release notes".to_string()));
        assert!(entry.download_url.is_some());
    }

    #[tokio::test]
    async fn test_get_plugin_from_github_not_found() {
        let mock_client = MockHttpClient::new();

        let registry = PluginRegistry::_with_client(mock_client);
        let result = registry
            .get_plugin_from_github("lpm-org/lpm-nonexistent", "nonexistent")
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_plugin_with_github_fallback() {
        // Mock GitHub response for first repo pattern
        let github_response = r#"{
            "tag_name": "v2.0.0",
            "name": "Release",
            "body": "Description",
            "published_at": "2024-01-01T00:00:00Z",
            "assets": [{
                "name": "lpm-plugin-darwin",
                "browser_download_url": "https://example.com/download",
                "size": 1000
            }]
        }"#;

        let mock_client = MockHttpClient::new().with_response(
            "https://api.github.com/repos/lpm-org/lpm-plugin/releases/latest",
            200,
            github_response.as_bytes().to_vec(),
        );

        let registry = PluginRegistry::_with_client(mock_client);
        let result = registry.get_plugin("plugin").await.unwrap();

        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.name, "plugin");
        assert_eq!(entry.version, "2.0.0");
    }

    #[tokio::test]
    async fn test_get_latest_version() {
        let github_response = r#"{
            "tag_name": "v3.0.0",
            "name": "Release",
            "body": null,
            "published_at": "2024-01-01T00:00:00Z",
            "assets": []
        }"#;

        let mock_client = MockHttpClient::new().with_response(
            "https://api.github.com/repos/lpm-org/lpm-example/releases/latest",
            200,
            github_response.as_bytes().to_vec(),
        );

        let registry = PluginRegistry::_with_client(mock_client);
        let version = registry.get_latest_version("example").await.unwrap();

        assert!(version.is_some());
        assert_eq!(version.unwrap(), "3.0.0");
    }

    #[tokio::test]
    async fn test_http_response_is_success() {
        let response = HttpResponse {
            status: 200,
            body: vec![],
        };
        assert!(response.is_success());

        let response = HttpResponse {
            status: 404,
            body: vec![],
        };
        assert!(!response.is_success());
    }

    #[tokio::test]
    async fn test_http_response_json() {
        #[derive(Deserialize)]
        struct TestStruct {
            name: String,
        }

        let json_data = r#"{"name":"test"}"#;
        let response = HttpResponse {
            status: 200,
            body: json_data.as_bytes().to_vec(),
        };

        let result: TestStruct = response.json().await.unwrap();
        assert_eq!(result.name, "test");
    }

    #[test]
    fn test_reqwest_client_creation() {
        let client = ReqwestClient::new();
        let _client2 = client; // Test that it's movable
    }

    #[test]
    fn test_registry_with_client() {
        let mock_client = MockHttpClient::new();
        let _registry = PluginRegistry::_with_client(mock_client);
    }
}
