//! Mock implementations of service traits for testing

use super::traits::{CacheProvider, ConfigProvider, PackageClient, SearchProvider};
use async_trait::async_trait;
use crate::core::{LpmError, LpmResult};
use crate::luarocks::manifest::Manifest;
use crate::luarocks::rockspec::Rockspec;
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
        let data = std::fs::read(artifact_path).map_err(|e| {
            LpmError::Cache(format!("Failed to read artifact: {}", e))
        })?;
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
        self.latest_versions.lock().unwrap().insert(package, version);
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
