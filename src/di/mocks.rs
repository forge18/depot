//! Mock implementations of service traits for testing

use super::traits::{CacheProvider, ConfigProvider, GitHubProvider};
use crate::core::{DepotError, DepotResult};
use crate::github::{GitHubRelease, GitHubTag, ResolvedVersion};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Mock configuration provider for testing
///
/// # Example
///
/// ```
/// use depot::di::mocks::MockConfigProvider;
/// use depot::di::ConfigProvider;
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
    pub cache_dir: PathBuf,
    pub verify_checksums: bool,
    pub show_diffs_on_update: bool,
    pub resolution_strategy: String,
    pub checksum_algorithm: String,
    pub strict_conflicts: bool,
    pub lua_binary_source_url: Option<String>,
    pub supported_lua_versions: Option<Vec<String>>,
    pub github_api_url: String,
    pub github_token: Option<String>,
    pub github_fallback_chain: Vec<String>,
    pub strict_native_code: bool,
}

impl Default for MockConfigProvider {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from("/tmp/depot-test-cache"),
            verify_checksums: true,
            show_diffs_on_update: true,
            resolution_strategy: "highest".to_string(),
            checksum_algorithm: "blake3".to_string(),
            strict_conflicts: true,
            lua_binary_source_url: None,
            supported_lua_versions: None,
            github_api_url: "https://api.github.com".to_string(),
            github_token: None,
            github_fallback_chain: vec![
                "release".to_string(),
                "tag".to_string(),
                "branch".to_string(),
            ],
            strict_native_code: true,
        }
    }
}

impl ConfigProvider for MockConfigProvider {
    fn cache_dir(&self) -> DepotResult<PathBuf> {
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

    fn github_api_url(&self) -> &str {
        &self.github_api_url
    }

    fn github_token(&self) -> Option<String> {
        self.github_token.clone()
    }

    fn github_fallback_chain(&self) -> &[String] {
        &self.github_fallback_chain
    }

    fn strict_native_code(&self) -> bool {
        self.strict_native_code
    }
}

/// Mock cache provider for testing
///
/// Stores files in memory instead of on disk.
///
/// # Example
///
/// ```
/// use depot::di::mocks::MockCacheProvider;
/// use depot::di::CacheProvider;
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
    /// Simulate I/O errors on read/write operations
    pub simulate_io_error: bool,
    /// Simulate disk full errors on write operations
    pub simulate_disk_full: bool,
    /// Force checksum verification failures
    pub fail_checksum_verification: bool,
    /// Paths that should deny read access (permission errors)
    pub deny_read_access: Arc<Mutex<std::collections::HashSet<PathBuf>>>,
}

impl MockCacheProvider {
    /// Create a new mock cache provider
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
            simulate_io_error: false,
            simulate_disk_full: false,
            fail_checksum_verification: false,
            deny_read_access: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Add a file to the mock cache
    pub fn add_file(&self, path: PathBuf, content: Vec<u8>) {
        self.files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(path, content);
    }

    /// Get all files in the mock cache
    pub fn get_files(&self) -> HashMap<PathBuf, Vec<u8>> {
        self.files.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Add a path to the deny list (simulates permission errors)
    pub fn deny_read(&self, path: PathBuf) {
        self.deny_read_access
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(path);
    }

    /// Builder pattern: simulate I/O errors
    pub fn with_io_error(mut self) -> Self {
        self.simulate_io_error = true;
        self
    }

    /// Builder pattern: simulate disk full
    pub fn with_disk_full(mut self) -> Self {
        self.simulate_disk_full = true;
        self
    }

    /// Builder pattern: fail checksum verification
    pub fn with_checksum_failure(mut self) -> Self {
        self.fail_checksum_verification = true;
        self
    }
}

impl Default for MockCacheProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheProvider for MockCacheProvider {
    fn package_metadata_path(&self, package: &str, version: &str) -> PathBuf {
        PathBuf::from(format!("/tmp/packages/{}/{}/.metadata", package, version))
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
        self.files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(path)
    }

    fn read(&self, path: &Path) -> DepotResult<Vec<u8>> {
        // Simulate permission denied
        if self
            .deny_read_access
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(path)
        {
            return Err(DepotError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Permission denied: {}", path.display()),
            )));
        }

        // Simulate I/O error
        if self.simulate_io_error {
            return Err(DepotError::Io(std::io::Error::other("Simulated I/O error")));
        }

        self.files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
            .cloned()
            .ok_or_else(|| DepotError::Cache(format!("File not found: {}", path.display())))
    }

    fn write(&self, path: &Path, data: &[u8]) -> DepotResult<()> {
        // Simulate disk full
        if self.simulate_disk_full {
            return Err(DepotError::Io(std::io::Error::other(
                "No space left on device",
            )));
        }

        // Simulate I/O error
        if self.simulate_io_error {
            return Err(DepotError::Io(std::io::Error::other("Simulated I/O error")));
        }

        self.files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(path.to_path_buf(), data.to_vec());
        Ok(())
    }

    fn checksum(&self, path: &Path) -> DepotResult<String> {
        let data = self.read(path)?;
        let hash = blake3::hash(&data);
        Ok(format!("blake3:{}", hash.to_hex()))
    }

    fn verify_checksum(&self, path: &Path, expected: &str) -> DepotResult<bool> {
        // Simulate checksum verification failure
        if self.fail_checksum_verification {
            return Ok(false);
        }

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
    ) -> DepotResult<PathBuf> {
        let dest = self.rust_build_path(package, version, lua_version, target);
        let data = std::fs::read(artifact_path)
            .map_err(|e| DepotError::Cache(format!("Failed to read artifact: {}", e)))?;
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

/// Mock GitHub provider for testing
///
/// Allows pre-populating releases, tags, and resolved versions for deterministic testing.
///
/// # Example
///
/// ```no_run
/// use depot::di::mocks::MockGitHubProvider;
///
/// let github = MockGitHubProvider::new();
/// // github.add_release("owner", "repo", release);
/// ```
#[derive(Clone)]
pub struct MockGitHubProvider {
    releases: Arc<Mutex<HashMap<String, Vec<GitHubRelease>>>>,
    tags: Arc<Mutex<HashMap<String, Vec<GitHubTag>>>>,
    default_branches: Arc<Mutex<HashMap<String, String>>>,
    tarballs: Arc<Mutex<HashMap<String, PathBuf>>>,
    file_contents: Arc<Mutex<HashMap<String, String>>>,
    /// Simulate API rate limit errors
    pub simulate_rate_limit: bool,
    /// Repositories that should return 404
    pub missing_repos: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl MockGitHubProvider {
    /// Create a new mock GitHub provider
    pub fn new() -> Self {
        Self {
            releases: Arc::new(Mutex::new(HashMap::new())),
            tags: Arc::new(Mutex::new(HashMap::new())),
            default_branches: Arc::new(Mutex::new(HashMap::new())),
            tarballs: Arc::new(Mutex::new(HashMap::new())),
            file_contents: Arc::new(Mutex::new(HashMap::new())),
            simulate_rate_limit: false,
            missing_repos: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Add a release for a repository
    pub fn add_release(&self, owner: &str, repo: &str, release: GitHubRelease) {
        let key = format!("{}/{}", owner, repo);
        self.releases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(key)
            .or_default()
            .push(release);
    }

    /// Add a tag for a repository
    pub fn add_tag(&self, owner: &str, repo: &str, tag: GitHubTag) {
        let key = format!("{}/{}", owner, repo);
        self.tags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(key)
            .or_default()
            .push(tag);
    }

    /// Set the default branch for a repository
    pub fn set_default_branch(&self, owner: &str, repo: &str, branch: String) {
        let key = format!("{}/{}", owner, repo);
        self.default_branches
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, branch);
    }

    /// Add a tarball download path
    pub fn add_tarball(&self, owner: &str, repo: &str, ref_: &str, path: PathBuf) {
        let key = format!("{}/{}/{}", owner, repo, ref_);
        self.tarballs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, path);
    }

    /// Add file content for a repository file
    pub fn add_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
        content: String,
    ) {
        let key = format!("{}/{}/{}@{}", owner, repo, path, ref_);
        self.file_contents
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, content);
    }

    /// Mark a repository as missing (returns 404)
    pub fn add_missing_repo(&self, owner: &str, repo: &str) {
        let key = format!("{}/{}", owner, repo);
        self.missing_repos
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key);
    }

    /// Builder pattern: simulate rate limit errors
    pub fn with_rate_limit(mut self) -> Self {
        self.simulate_rate_limit = true;
        self
    }
}

impl Default for MockGitHubProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitHubProvider for MockGitHubProvider {
    async fn get_releases(&self, owner: &str, repo: &str) -> DepotResult<Vec<GitHubRelease>> {
        if self.simulate_rate_limit {
            return Err(DepotError::Package("API rate limit exceeded".to_string()));
        }

        let key = format!("{}/{}", owner, repo);
        if self
            .missing_repos
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&key)
        {
            return Err(DepotError::Package(format!(
                "Repository not found: {}",
                key
            )));
        }

        Ok(self
            .releases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
            .unwrap_or_default())
    }

    async fn get_latest_release(&self, owner: &str, repo: &str) -> DepotResult<GitHubRelease> {
        let releases = self.get_releases(owner, repo).await?;
        releases
            .into_iter()
            .find(|r| !r.draft && !r.prerelease)
            .ok_or_else(|| DepotError::Package("No releases found".to_string()))
    }

    async fn get_tags(&self, owner: &str, repo: &str) -> DepotResult<Vec<GitHubTag>> {
        if self.simulate_rate_limit {
            return Err(DepotError::Package("API rate limit exceeded".to_string()));
        }

        let key = format!("{}/{}", owner, repo);
        if self
            .missing_repos
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&key)
        {
            return Err(DepotError::Package(format!(
                "Repository not found: {}",
                key
            )));
        }

        Ok(self
            .tags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
            .unwrap_or_default())
    }

    async fn get_default_branch(&self, owner: &str, repo: &str) -> DepotResult<String> {
        if self.simulate_rate_limit {
            return Err(DepotError::Package("API rate limit exceeded".to_string()));
        }

        let key = format!("{}/{}", owner, repo);
        if self
            .missing_repos
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&key)
        {
            return Err(DepotError::Package(format!(
                "Repository not found: {}",
                key
            )));
        }

        Ok(self
            .default_branches
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
            .unwrap_or_else(|| "main".to_string()))
    }

    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
    ) -> DepotResult<String> {
        if self.simulate_rate_limit {
            return Err(DepotError::Package("API rate limit exceeded".to_string()));
        }

        let key = format!("{}/{}/{}@{}", owner, repo, path, ref_);
        self.file_contents
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
            .ok_or_else(|| DepotError::Package(format!("File not found: {}", path)))
    }

    async fn download_tarball(&self, owner: &str, repo: &str, ref_: &str) -> DepotResult<PathBuf> {
        if self.simulate_rate_limit {
            return Err(DepotError::Package("API rate limit exceeded".to_string()));
        }

        let key = format!("{}/{}/{}", owner, repo, ref_);
        self.tarballs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
            .ok_or_else(|| DepotError::Package(format!("Tarball not found for ref: {}", ref_)))
    }

    async fn resolve_version(
        &self,
        owner: &str,
        repo: &str,
        version_spec: Option<&str>,
        fallback_chain: &[String],
    ) -> DepotResult<ResolvedVersion> {
        if self.simulate_rate_limit {
            return Err(DepotError::Package("API rate limit exceeded".to_string()));
        }

        // If specific version requested, try to find it
        if let Some(version) = version_spec {
            // Try as release tag
            if let Ok(releases) = self.get_releases(owner, repo).await {
                if let Some(r) = releases.iter().find(|r| r.tag_name == version) {
                    return Ok(ResolvedVersion {
                        ref_type: crate::github::RefType::Release,
                        ref_value: r.tag_name.clone(),
                        commit_sha: "mock-sha".to_string(),
                        tarball_url: r.tarball_url.clone(),
                    });
                }
            }

            // Try as tag
            if let Ok(tags) = self.get_tags(owner, repo).await {
                if let Some(tag) = tags.iter().find(|t| t.name == version) {
                    return Ok(ResolvedVersion {
                        ref_type: crate::github::RefType::Tag,
                        ref_value: tag.name.clone(),
                        commit_sha: tag.commit.sha.clone(),
                        tarball_url: tag.tarball_url.clone(),
                    });
                }
            }

            return Err(DepotError::Package(format!(
                "Version {} not found",
                version
            )));
        }

        // Use fallback chain
        for strategy in fallback_chain {
            match strategy.as_str() {
                "release" => {
                    if let Ok(release) = self.get_latest_release(owner, repo).await {
                        return Ok(ResolvedVersion {
                            ref_type: crate::github::RefType::Release,
                            ref_value: release.tag_name.clone(),
                            commit_sha: "mock-sha".to_string(),
                            tarball_url: release.tarball_url.clone(),
                        });
                    }
                }
                "tag" => {
                    if let Ok(tags) = self.get_tags(owner, repo).await {
                        if let Some(tag) = tags.first() {
                            return Ok(ResolvedVersion {
                                ref_type: crate::github::RefType::Tag,
                                ref_value: tag.name.clone(),
                                commit_sha: tag.commit.sha.clone(),
                                tarball_url: tag.tarball_url.clone(),
                            });
                        }
                    }
                }
                "branch" => {
                    let default_branch = self.get_default_branch(owner, repo).await?;
                    return Ok(ResolvedVersion {
                        ref_type: crate::github::RefType::Branch,
                        ref_value: default_branch.clone(),
                        commit_sha: "mock-sha".to_string(),
                        tarball_url: format!(
                            "https://api.github.com/repos/{}/{}/tarball/{}",
                            owner, repo, default_branch
                        ),
                    });
                }
                _ => continue,
            }
        }

        Err(DepotError::Package("Could not resolve version".to_string()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::di::traits::{CacheProvider, ConfigProvider};

    #[test]
    fn test_mock_config_provider_default() {
        let config = MockConfigProvider::default();

        assert_eq!(config.github_api_url(), "https://api.github.com");
        assert!(config.verify_checksums());
        assert!(config.show_diffs_on_update());
        assert_eq!(config.resolution_strategy(), "highest");
        assert_eq!(config.checksum_algorithm(), "blake3");
        assert!(config.strict_conflicts());
        assert_eq!(config.lua_binary_source_url(), None);
        assert_eq!(config.supported_lua_versions(), None);
        assert!(config.strict_native_code());
    }

    #[test]
    fn test_mock_config_provider_custom() {
        let config = MockConfigProvider {
            cache_dir: PathBuf::from("/custom/cache"),
            verify_checksums: false,
            show_diffs_on_update: false,
            resolution_strategy: "lowest".to_string(),
            checksum_algorithm: "sha256".to_string(),
            strict_conflicts: false,
            lua_binary_source_url: Some("https://lua.org".to_string()),
            supported_lua_versions: Some(vec!["5.1".to_string(), "5.4".to_string()]),
            github_api_url: "https://github.enterprise.com/api".to_string(),
            github_token: Some("ghp_test123".to_string()),
            github_fallback_chain: vec!["tag".to_string()],
            strict_native_code: false,
        };

        assert_eq!(config.github_api_url(), "https://github.enterprise.com/api");
        assert_eq!(config.github_token(), Some("ghp_test123".to_string()));
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
        assert!(!config.strict_native_code());
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
    fn test_mock_cache_provider_package_metadata_path() {
        let cache = MockCacheProvider::new();
        let path = cache.package_metadata_path("test-package", "1.0.0");

        assert_eq!(
            path,
            PathBuf::from("/tmp/packages/test-package/1.0.0/.metadata")
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
    fn test_mock_cache_builder_chaining() {
        let cache = MockCacheProvider::new()
            .with_io_error()
            .with_disk_full()
            .with_checksum_failure();
        assert!(cache.simulate_io_error);
        assert!(cache.simulate_disk_full);
        assert!(cache.fail_checksum_verification);
    }
}
