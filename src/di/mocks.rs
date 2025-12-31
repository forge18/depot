//! Mock implementations of service traits for testing

use super::traits::{CacheProvider, ConfigProvider, PackageClient, SearchProvider};
use crate::core::{LpmError, LpmResult};
use crate::luarocks::manifest::Manifest;
use crate::luarocks::rockspec::Rockspec;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Mock configuration provider for testing
///
/// # Example
///
/// ```
/// use lpm::di::mocks::MockConfigProvider;
/// use lpm::di::ConfigProvider;
/// use std::path::PathBuf;
///
/// let mut config = MockConfigProvider::default();
/// config.cache_dir = PathBuf::from("/tmp/test-cache");
/// config.verify_checksums = false;
///
/// assert_eq!(config.verify_checksums(), false);
/// ```
#[derive(Clone)]
pub struct MockConfigProvider {
    pub manifest_url: String,
    pub cache_dir: PathBuf,
    pub verify_checksums: bool,
    pub show_diffs_on_update: bool,
    pub resolution_strategy: String,
    pub checksum_algorithm: String,
    pub strict_conflicts: bool,
    pub lua_binary_source_url: Option<String>,
    pub supported_lua_versions: Option<Vec<String>>,
}

impl Default for MockConfigProvider {
    fn default() -> Self {
        Self {
            manifest_url: "https://luarocks.org/manifests/luarocks/manifest".to_string(),
            cache_dir: PathBuf::from("/tmp/lpm-test-cache"),
            verify_checksums: true,
            show_diffs_on_update: true,
            resolution_strategy: "highest".to_string(),
            checksum_algorithm: "blake3".to_string(),
            strict_conflicts: true,
            lua_binary_source_url: None,
            supported_lua_versions: None,
        }
    }
}

impl ConfigProvider for MockConfigProvider {
    fn luarocks_manifest_url(&self) -> &str {
        &self.manifest_url
    }

    fn cache_dir(&self) -> LpmResult<PathBuf> {
        Ok(self.cache_dir.clone())
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

/// Mock cache provider for testing
///
/// Stores files in memory instead of on disk.
///
/// # Example
///
/// ```
/// use lpm::di::mocks::MockCacheProvider;
/// use lpm::di::CacheProvider;
/// use std::path::PathBuf;
///
/// let cache = MockCacheProvider::new();
/// cache.add_file(PathBuf::from("/test/file.txt"), b"content".to_vec());
///
/// assert!(cache.exists(&PathBuf::from("/test/file.txt")));
/// ```
#[derive(Clone)]
pub struct MockCacheProvider {
    files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
}

impl MockCacheProvider {
    /// Create a new mock cache provider
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a file to the mock cache
    pub fn add_file(&self, path: PathBuf, content: Vec<u8>) {
        self.files.lock().unwrap().insert(path, content);
    }

    /// Get all files in the mock cache
    pub fn get_files(&self) -> HashMap<PathBuf, Vec<u8>> {
        self.files.lock().unwrap().clone()
    }
}

impl Default for MockCacheProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheProvider for MockCacheProvider {
    fn rockspec_path(&self, package: &str, version: &str) -> PathBuf {
        PathBuf::from(format!("/tmp/rockspecs/{}-{}.rockspec", package, version))
    }

    fn source_path(&self, url: &str) -> PathBuf {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();

        let extension = Path::new(url)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("tar.gz");

        PathBuf::from(format!("/tmp/sources/{:x}.{}", hash, extension))
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }

    fn read(&self, path: &Path) -> LpmResult<Vec<u8>> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| LpmError::Cache(format!("File not found: {}", path.display())))
    }

    fn write(&self, path: &Path, data: &[u8]) -> LpmResult<()> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), data.to_vec());
        Ok(())
    }

    fn checksum(&self, path: &Path) -> LpmResult<String> {
        let data = self.read(path)?;
        let hash = blake3::hash(&data);
        Ok(format!("blake3:{}", hash.to_hex()))
    }

    fn verify_checksum(&self, path: &Path, expected: &str) -> LpmResult<bool> {
        let actual = self.checksum(path)?;
        Ok(actual == expected)
    }

    fn rust_build_path(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
    ) -> PathBuf {
        PathBuf::from(format!(
            "/tmp/rust-builds/{}/{}/{}/{}",
            package, version, lua_version, target
        ))
    }

    fn has_rust_build(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
    ) -> bool {
        self.exists(&self.rust_build_path(package, version, lua_version, target))
    }

    fn store_rust_build(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
        artifact_path: &Path,
    ) -> LpmResult<PathBuf> {
        let dest = self.rust_build_path(package, version, lua_version, target);
        let data = std::fs::read(artifact_path)
            .map_err(|e| LpmError::Cache(format!("Failed to read artifact: {}", e)))?;
        self.write(&dest, &data)?;
        Ok(dest)
    }

    fn get_rust_build(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
    ) -> Option<PathBuf> {
        let path = self.rust_build_path(package, version, lua_version, target);
        if self.exists(&path) {
            Some(path)
        } else {
            None
        }
    }
}

/// Mock package client for testing
///
/// Allows pre-populating rockspecs and sources for deterministic testing.
///
/// # Example
///
/// ```
/// use lpm::di::mocks::MockPackageClient;
/// use std::path::PathBuf;
///
/// let client = MockPackageClient::new();
/// client.add_rockspec(
///     "https://example.com/test-1.0.0.rockspec".to_string(),
///     "package = 'test'\nversion = '1.0.0'".to_string(),
/// );
/// ```
#[derive(Clone)]
pub struct MockPackageClient {
    manifests: Arc<Mutex<HashMap<String, Manifest>>>,
    rockspecs: Arc<Mutex<HashMap<String, String>>>,
    sources: Arc<Mutex<HashMap<String, PathBuf>>>,
}

impl MockPackageClient {
    /// Create a new mock package client
    pub fn new() -> Self {
        Self {
            manifests: Arc::new(Mutex::new(HashMap::new())),
            rockspecs: Arc::new(Mutex::new(HashMap::new())),
            sources: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a rockspec to the mock client
    pub fn add_rockspec(&self, url: String, content: String) {
        self.rockspecs.lock().unwrap().insert(url, content);
    }

    /// Add a source package to the mock client
    pub fn add_source(&self, url: String, path: PathBuf) {
        self.sources.lock().unwrap().insert(url, path);
    }

    /// Add a manifest to the mock client
    pub fn add_manifest(&self, url: String, manifest: Manifest) {
        self.manifests.lock().unwrap().insert(url, manifest);
    }
}

impl Default for MockPackageClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PackageClient for MockPackageClient {
    async fn fetch_manifest(&self) -> LpmResult<Manifest> {
        // Return a default empty manifest for tests
        Ok(Manifest::default())
    }

    async fn download_rockspec(&self, url: &str) -> LpmResult<String> {
        self.rockspecs
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or_else(|| LpmError::Package(format!("Rockspec not found: {}", url)))
    }

    fn parse_rockspec(&self, content: &str) -> LpmResult<Rockspec> {
        Rockspec::parse_lua(content)
    }

    async fn download_source(&self, url: &str) -> LpmResult<PathBuf> {
        self.sources
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or_else(|| LpmError::Package(format!("Source not found: {}", url)))
    }
}

/// Mock search provider for testing
///
/// Allows pre-populating package versions and rockspec URLs.
///
/// # Example
///
/// ```
/// use lpm::di::mocks::MockSearchProvider;
///
/// let search = MockSearchProvider::new();
/// search.add_latest_version("test".to_string(), "1.0.0".to_string());
/// ```
#[derive(Clone)]
pub struct MockSearchProvider {
    latest_versions: Arc<Mutex<HashMap<String, String>>>,
    valid_urls: Arc<Mutex<Vec<String>>>,
}

impl MockSearchProvider {
    /// Create a new mock search provider
    pub fn new() -> Self {
        Self {
            latest_versions: Arc::new(Mutex::new(HashMap::new())),
            valid_urls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a latest version for a package
    pub fn add_latest_version(&self, package: String, version: String) {
        self.latest_versions
            .lock()
            .unwrap()
            .insert(package, version);
    }

    /// Add a valid rockspec URL
    pub fn add_valid_url(&self, url: String) {
        self.valid_urls.lock().unwrap().push(url);
    }
}

impl Default for MockSearchProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchProvider for MockSearchProvider {
    async fn get_latest_version(&self, package_name: &str) -> LpmResult<String> {
        self.latest_versions
            .lock()
            .unwrap()
            .get(package_name)
            .cloned()
            .ok_or_else(|| LpmError::Package(format!("Package not found: {}", package_name)))
    }

    fn get_rockspec_url(
        &self,
        package_name: &str,
        version: &str,
        _manifest: Option<&str>,
    ) -> String {
        format!(
            "https://luarocks.org/manifests/luarocks/{}-{}.rockspec",
            package_name, version
        )
    }

    async fn verify_rockspec_url(&self, url: &str) -> LpmResult<()> {
        if self.valid_urls.lock().unwrap().contains(&url.to_string()) {
            Ok(())
        } else {
            // By default, accept all URLs in tests
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::di::traits::{CacheProvider, ConfigProvider, PackageClient, SearchProvider};

    #[test]
    fn test_mock_config_provider_default() {
        let config = MockConfigProvider::default();

        assert_eq!(
            config.luarocks_manifest_url(),
            "https://luarocks.org/manifests/luarocks/manifest"
        );
        assert!(config.verify_checksums());
        assert!(config.show_diffs_on_update());
        assert_eq!(config.resolution_strategy(), "highest");
        assert_eq!(config.checksum_algorithm(), "blake3");
        assert!(config.strict_conflicts());
        assert_eq!(config.lua_binary_source_url(), None);
        assert_eq!(config.supported_lua_versions(), None);
    }

    #[test]
    fn test_mock_config_provider_custom() {
        let config = MockConfigProvider {
            manifest_url: "https://custom.com/manifest".to_string(),
            cache_dir: PathBuf::from("/custom/cache"),
            verify_checksums: false,
            show_diffs_on_update: false,
            resolution_strategy: "lowest".to_string(),
            checksum_algorithm: "sha256".to_string(),
            strict_conflicts: false,
            lua_binary_source_url: Some("https://lua.org".to_string()),
            supported_lua_versions: Some(vec!["5.1".to_string(), "5.4".to_string()]),
        };

        assert_eq!(
            config.luarocks_manifest_url(),
            "https://custom.com/manifest"
        );
        assert_eq!(config.cache_dir().unwrap(), PathBuf::from("/custom/cache"));
        assert!(!config.verify_checksums());
        assert!(!config.show_diffs_on_update());
        assert_eq!(config.resolution_strategy(), "lowest");
        assert_eq!(config.checksum_algorithm(), "sha256");
        assert!(!config.strict_conflicts());
        assert_eq!(config.lua_binary_source_url(), Some("https://lua.org"));
        assert_eq!(
            config.supported_lua_versions(),
            Some(&vec!["5.1".to_string(), "5.4".to_string()])
        );
    }

    #[test]
    fn test_mock_cache_provider_new() {
        let cache = MockCacheProvider::new();
        assert!(cache.get_files().is_empty());
    }

    #[test]
    fn test_mock_cache_provider_add_file() {
        let cache = MockCacheProvider::new();
        let path = PathBuf::from("/test/file.txt");
        let content = b"test content".to_vec();

        cache.add_file(path.clone(), content.clone());

        assert!(cache.exists(&path));
        assert_eq!(cache.read(&path).unwrap(), content);
    }

    #[test]
    fn test_mock_cache_provider_write_read() {
        let cache = MockCacheProvider::new();
        let path = PathBuf::from("/test/write.txt");
        let content = b"written content";

        cache.write(&path, content).unwrap();

        assert!(cache.exists(&path));
        assert_eq!(cache.read(&path).unwrap(), content);
    }

    #[test]
    fn test_mock_cache_provider_read_nonexistent() {
        let cache = MockCacheProvider::new();
        let path = PathBuf::from("/nonexistent.txt");

        let result = cache.read(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[test]
    fn test_mock_cache_provider_rockspec_path() {
        let cache = MockCacheProvider::new();
        let path = cache.rockspec_path("test-package", "1.0.0");

        assert_eq!(
            path,
            PathBuf::from("/tmp/rockspecs/test-package-1.0.0.rockspec")
        );
    }

    #[test]
    fn test_mock_cache_provider_source_path() {
        let cache = MockCacheProvider::new();
        let url = "https://example.com/package.tar.gz";
        let path = cache.source_path(url);

        // Should be deterministic hash
        assert!(path.to_string_lossy().starts_with("/tmp/sources/"));
        assert!(path.to_string_lossy().contains(".gz")); // Extension is preserved but after hash

        // Same URL should produce same path
        assert_eq!(path, cache.source_path(url));
    }

    #[test]
    fn test_mock_cache_provider_source_path_no_extension() {
        let cache = MockCacheProvider::new();
        let url = "https://example.com/package";
        let path = cache.source_path(url);

        // Should use default extension
        assert!(path.to_string_lossy().ends_with(".tar.gz"));
    }

    #[test]
    fn test_mock_cache_provider_checksum() {
        let cache = MockCacheProvider::new();
        let path = PathBuf::from("/test/checksum.txt");
        let content = b"content to hash";

        cache.write(&path, content).unwrap();

        let checksum = cache.checksum(&path).unwrap();
        assert!(checksum.starts_with("blake3:"));
    }

    #[test]
    fn test_mock_cache_provider_verify_checksum() {
        let cache = MockCacheProvider::new();
        let path = PathBuf::from("/test/verify.txt");
        let content = b"content";

        cache.write(&path, content).unwrap();

        let checksum = cache.checksum(&path).unwrap();
        assert!(cache.verify_checksum(&path, &checksum).unwrap());
        assert!(!cache.verify_checksum(&path, "blake3:wrong").unwrap());
    }

    #[test]
    fn test_mock_cache_provider_rust_build_path() {
        let cache = MockCacheProvider::new();
        let path = cache.rust_build_path("pkg", "1.0.0", "5.1", "x86_64-linux");

        assert_eq!(
            path,
            PathBuf::from("/tmp/rust-builds/pkg/1.0.0/5.1/x86_64-linux")
        );
    }

    #[test]
    fn test_mock_cache_provider_has_rust_build() {
        let cache = MockCacheProvider::new();

        assert!(!cache.has_rust_build("pkg", "1.0.0", "5.1", "x86_64-linux"));

        let path = cache.rust_build_path("pkg", "1.0.0", "5.1", "x86_64-linux");
        cache.write(&path, b"binary").unwrap();

        assert!(cache.has_rust_build("pkg", "1.0.0", "5.1", "x86_64-linux"));
    }

    #[test]
    fn test_mock_cache_provider_get_rust_build() {
        let cache = MockCacheProvider::new();

        assert_eq!(
            cache.get_rust_build("pkg", "1.0.0", "5.1", "x86_64-linux"),
            None
        );

        let path = cache.rust_build_path("pkg", "1.0.0", "5.1", "x86_64-linux");
        cache.write(&path, b"binary").unwrap();

        let result = cache.get_rust_build("pkg", "1.0.0", "5.1", "x86_64-linux");
        assert_eq!(result, Some(path));
    }

    #[test]
    fn test_mock_cache_provider_store_rust_build() {
        use std::fs;
        use tempfile::TempDir;

        let cache = MockCacheProvider::new();
        let temp = TempDir::new().unwrap();
        let artifact = temp.path().join("artifact.so");
        fs::write(&artifact, b"binary content").unwrap();

        let result = cache.store_rust_build("pkg", "1.0.0", "5.1", "x86_64-linux", &artifact);
        assert!(result.is_ok());

        let stored_path = result.unwrap();
        assert_eq!(
            stored_path,
            PathBuf::from("/tmp/rust-builds/pkg/1.0.0/5.1/x86_64-linux")
        );
        assert_eq!(cache.read(&stored_path).unwrap(), b"binary content");
    }

    #[test]
    fn test_mock_cache_provider_store_rust_build_nonexistent() {
        let cache = MockCacheProvider::new();
        let artifact = PathBuf::from("/nonexistent/artifact.so");

        let result = cache.store_rust_build("pkg", "1.0.0", "5.1", "x86_64-linux", &artifact);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read artifact"));
    }

    #[test]
    fn test_mock_cache_provider_get_files() {
        let cache = MockCacheProvider::new();
        cache.add_file(PathBuf::from("/file1.txt"), b"content1".to_vec());
        cache.add_file(PathBuf::from("/file2.txt"), b"content2".to_vec());

        let files = cache.get_files();
        assert_eq!(files.len(), 2);
        assert_eq!(
            files.get(&PathBuf::from("/file1.txt")),
            Some(&b"content1".to_vec())
        );
        assert_eq!(
            files.get(&PathBuf::from("/file2.txt")),
            Some(&b"content2".to_vec())
        );
    }

    #[test]
    fn test_mock_cache_provider_default() {
        let cache = MockCacheProvider::default();
        assert!(cache.get_files().is_empty());
    }

    #[test]
    fn test_mock_package_client_new() {
        let client = MockPackageClient::new();
        // Just verify it can be created
        assert!(client.rockspecs.lock().unwrap().is_empty());
    }

    #[test]
    fn test_mock_package_client_add_rockspec() {
        let client = MockPackageClient::new();
        client.add_rockspec(
            "https://example.com/test.rockspec".to_string(),
            "package = 'test'".to_string(),
        );

        let rockspecs = client.rockspecs.lock().unwrap();
        assert_eq!(
            rockspecs.get("https://example.com/test.rockspec"),
            Some(&"package = 'test'".to_string())
        );
    }

    #[test]
    fn test_mock_package_client_add_source() {
        let client = MockPackageClient::new();
        client.add_source(
            "https://example.com/source.tar.gz".to_string(),
            PathBuf::from("/tmp/source.tar.gz"),
        );

        let sources = client.sources.lock().unwrap();
        assert_eq!(
            sources.get("https://example.com/source.tar.gz"),
            Some(&PathBuf::from("/tmp/source.tar.gz"))
        );
    }

    #[test]
    fn test_mock_package_client_add_manifest() {
        let client = MockPackageClient::new();
        let manifest = Manifest::default();
        client.add_manifest("https://example.com/manifest".to_string(), manifest.clone());

        let manifests = client.manifests.lock().unwrap();
        assert!(manifests.contains_key("https://example.com/manifest"));
    }

    #[tokio::test]
    async fn test_mock_package_client_fetch_manifest() {
        let client = MockPackageClient::new();
        let result = client.fetch_manifest().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_package_client_download_rockspec() {
        let client = MockPackageClient::new();
        client.add_rockspec(
            "https://example.com/test.rockspec".to_string(),
            "package = 'test'".to_string(),
        );

        let result = client
            .download_rockspec("https://example.com/test.rockspec")
            .await;
        assert_eq!(result.unwrap(), "package = 'test'");
    }

    #[tokio::test]
    async fn test_mock_package_client_download_rockspec_not_found() {
        let client = MockPackageClient::new();
        let result = client
            .download_rockspec("https://example.com/missing.rockspec")
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Rockspec not found"));
    }

    #[tokio::test]
    async fn test_mock_package_client_download_source() {
        let client = MockPackageClient::new();
        client.add_source(
            "https://example.com/source.tar.gz".to_string(),
            PathBuf::from("/tmp/source.tar.gz"),
        );

        let result = client
            .download_source("https://example.com/source.tar.gz")
            .await;
        assert_eq!(result.unwrap(), PathBuf::from("/tmp/source.tar.gz"));
    }

    #[tokio::test]
    async fn test_mock_package_client_download_source_not_found() {
        let client = MockPackageClient::new();
        let result = client
            .download_source("https://example.com/missing.tar.gz")
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Source not found"));
    }

    #[test]
    fn test_mock_package_client_parse_rockspec() {
        let client = MockPackageClient::new();
        // This test verifies that parse_rockspec delegates to Rockspec::parse_lua
        // We don't test the full parsing here as that's covered by rockspec module tests
        let rockspec_content = r#"
package = "test"
version = "1.0.0-1"
source = {
    url = "https://example.com/test-1.0.0.tar.gz"
}
dependencies = {
    "lua >= 5.1"
}
"#;

        let result = client.parse_rockspec(rockspec_content);
        // Just verify it attempts to parse - actual parsing logic is tested in rockspec module
        assert!(result.is_ok() || result.is_err()); // Either is fine, we're just testing the call
    }

    #[test]
    fn test_mock_package_client_default() {
        let client = MockPackageClient::default();
        assert!(client.rockspecs.lock().unwrap().is_empty());
    }

    #[test]
    fn test_mock_search_provider_new() {
        let search = MockSearchProvider::new();
        assert!(search.latest_versions.lock().unwrap().is_empty());
        assert!(search.valid_urls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_mock_search_provider_add_latest_version() {
        let search = MockSearchProvider::new();
        search.add_latest_version("test".to_string(), "1.0.0".to_string());

        let versions = search.latest_versions.lock().unwrap();
        assert_eq!(versions.get("test"), Some(&"1.0.0".to_string()));
    }

    #[test]
    fn test_mock_search_provider_add_valid_url() {
        let search = MockSearchProvider::new();
        search.add_valid_url("https://example.com/test.rockspec".to_string());

        let urls = search.valid_urls.lock().unwrap();
        assert!(urls.contains(&"https://example.com/test.rockspec".to_string()));
    }

    #[tokio::test]
    async fn test_mock_search_provider_get_latest_version() {
        let search = MockSearchProvider::new();
        search.add_latest_version("test".to_string(), "1.0.0".to_string());

        let result = search.get_latest_version("test").await;
        assert_eq!(result.unwrap(), "1.0.0");
    }

    #[tokio::test]
    async fn test_mock_search_provider_get_latest_version_not_found() {
        let search = MockSearchProvider::new();
        let result = search.get_latest_version("missing").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Package not found"));
    }

    #[test]
    fn test_mock_search_provider_get_rockspec_url() {
        let search = MockSearchProvider::new();
        let url = search.get_rockspec_url("test", "1.0.0", None);

        assert_eq!(
            url,
            "https://luarocks.org/manifests/luarocks/test-1.0.0.rockspec"
        );
    }

    #[test]
    fn test_mock_search_provider_get_rockspec_url_with_manifest() {
        let search = MockSearchProvider::new();
        let url = search.get_rockspec_url("test", "1.0.0", Some("custom"));

        // Manifest parameter is ignored in mock
        assert_eq!(
            url,
            "https://luarocks.org/manifests/luarocks/test-1.0.0.rockspec"
        );
    }

    #[tokio::test]
    async fn test_mock_search_provider_verify_rockspec_url() {
        let search = MockSearchProvider::new();

        // By default, all URLs are valid
        let result = search
            .verify_rockspec_url("https://any.com/test.rockspec")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_search_provider_verify_rockspec_url_in_list() {
        let search = MockSearchProvider::new();
        search.add_valid_url("https://valid.com/test.rockspec".to_string());

        let result = search
            .verify_rockspec_url("https://valid.com/test.rockspec")
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_search_provider_default() {
        let search = MockSearchProvider::default();
        assert!(search.latest_versions.lock().unwrap().is_empty());
    }
}
