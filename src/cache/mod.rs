use crate::core::path::{cache_dir, ensure_dir};
use crate::core::{LpmError, LpmResult};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Checksum algorithm for verifying package integrity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChecksumAlgorithm {
    /// SHA-256 (legacy, for backward compatibility)
    Sha256,
    /// BLAKE3 (default, faster and more secure)
    #[default]
    Blake3,
}

impl ChecksumAlgorithm {
    /// Parse algorithm from a prefixed checksum string
    pub fn from_checksum(checksum: &str) -> Self {
        if checksum.starts_with("blake3:") {
            ChecksumAlgorithm::Blake3
        } else if checksum.starts_with("sha256:") {
            ChecksumAlgorithm::Sha256
        } else {
            // Default to BLAKE3 for unprefixed checksums
            ChecksumAlgorithm::Blake3
        }
    }
}

/// Package cache manager
#[derive(Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    /// Create a new cache instance
    pub fn new(cache_root: PathBuf) -> LpmResult<Self> {
        ensure_dir(&cache_root)?;
        Ok(Self { root: cache_root })
    }

    /// Get the default cache directory
    pub fn default_cache() -> LpmResult<Self> {
        Self::new(cache_dir()?)
    }

    /// Get the LuaRocks cache directory
    pub fn luarocks_dir(&self) -> PathBuf {
        self.root.join("luarocks")
    }

    /// Get the rockspecs cache directory
    pub fn rockspecs_dir(&self) -> PathBuf {
        self.luarocks_dir().join("rockspecs")
    }

    /// Get the sources cache directory
    pub fn sources_dir(&self) -> PathBuf {
        self.luarocks_dir().join("sources")
    }

    /// Get the Rust builds cache directory
    pub fn rust_builds_dir(&self) -> PathBuf {
        self.root.join("rust-builds")
    }

    /// Initialize cache directory structure
    pub fn init(&self) -> LpmResult<()> {
        ensure_dir(&self.luarocks_dir())?;
        ensure_dir(&self.rockspecs_dir())?;
        ensure_dir(&self.sources_dir())?;
        ensure_dir(&self.rust_builds_dir())?;
        Ok(())
    }

    /// Get the cached path for a rockspec file
    pub fn rockspec_path(&self, package: &str, version: &str) -> PathBuf {
        let filename = format!("{}-{}.rockspec", package, version);
        self.rockspecs_dir().join(filename)
    }

    /// Get the cached path for a source archive
    pub fn source_path(&self, url: &str) -> PathBuf {
        // Use URL hash as filename to avoid path issues
        let hash = Self::url_hash(url);
        let extension = Path::new(url)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("tar.gz");
        self.sources_dir().join(format!("{}.{}", hash, extension))
    }

    /// Check if a file exists in cache
    pub fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    /// Read a file from cache
    pub fn read(&self, path: &Path) -> LpmResult<Vec<u8>> {
        fs::read(path).map_err(|e| {
            LpmError::Cache(format!(
                "Failed to read from cache: {}: {}",
                path.display(),
                e
            ))
        })
    }

    /// Write a file to cache
    pub fn write(&self, path: &Path, data: &[u8]) -> LpmResult<()> {
        if let Some(parent) = path.parent() {
            ensure_dir(parent)?;
        }
        let mut file = fs::File::create(path).map_err(|e| {
            LpmError::Cache(format!(
                "Failed to create cache file: {}: {}",
                path.display(),
                e
            ))
        })?;
        file.write_all(data).map_err(|e| {
            LpmError::Cache(format!(
                "Failed to write to cache: {}: {}",
                path.display(),
                e
            ))
        })?;
        Ok(())
    }

    /// Calculate checksum of a file using the specified algorithm
    pub fn checksum_with_algorithm(path: &Path, algorithm: ChecksumAlgorithm) -> LpmResult<String> {
        let data = fs::read(path)?;
        match algorithm {
            ChecksumAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(&data);
                Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
            }
            ChecksumAlgorithm::Blake3 => {
                let hash = blake3::hash(&data);
                Ok(format!("blake3:{}", hash.to_hex()))
            }
        }
    }

    /// Calculate checksum of a file (defaults to BLAKE3)
    pub fn checksum(path: &Path) -> LpmResult<String> {
        Self::checksum_with_algorithm(path, ChecksumAlgorithm::default())
    }

    /// Verify a file's checksum matches the expected value (supports both SHA-256 and BLAKE3)
    pub fn verify_checksum(path: &Path, expected: &str) -> LpmResult<bool> {
        let algorithm = ChecksumAlgorithm::from_checksum(expected);
        let actual = Self::checksum_with_algorithm(path, algorithm)?;

        // Compare without prefix for backward compatibility
        let expected_hash = expected.split_once(':').map(|(_, h)| h).unwrap_or(expected);
        let actual_hash = actual.split_once(':').map(|(_, h)| h).unwrap_or(&actual);

        Ok(expected_hash == actual_hash)
    }

    /// Hash a URL for use as a filename
    fn url_hash(url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = hasher.finalize();
        hex::encode(&hash[..16]) // Use first 16 bytes for shorter filename
    }

    /// Get the cached path for a Rust build artifact
    /// Get the cache path for a Rust build, including Lua version
    ///
    /// Structure: rust_builds/{package}/{version}/{lua_version}/{target}/
    pub fn rust_build_path(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
    ) -> PathBuf {
        let dir = self
            .rust_builds_dir()
            .join(package)
            .join(version)
            .join(lua_version)
            .join(target);
        // Determine library extension based on target
        let extension = if target.contains("windows") {
            "dll"
        } else if target.contains("darwin") || target.contains("apple") {
            "dylib"
        } else {
            "so"
        };
        dir.join(format!("lib{}.{}", package.replace('-', "_"), extension))
    }

    /// Check if a Rust build artifact is cached
    pub fn has_rust_build(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
    ) -> bool {
        self.exists(&self.rust_build_path(package, version, lua_version, target))
    }

    /// Store a Rust build artifact in cache
    ///
    /// Caches are stored per Lua version to support multiple Lua installations
    pub fn store_rust_build(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
        artifact_path: &Path,
    ) -> LpmResult<PathBuf> {
        let cache_path = self.rust_build_path(package, version, lua_version, target);

        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            ensure_dir(parent)?;
        }

        // Copy artifact to cache
        fs::copy(artifact_path, &cache_path).map_err(|e| {
            LpmError::Cache(format!("Failed to copy build artifact to cache: {}", e))
        })?;

        Ok(cache_path)
    }

    /// Get cached Rust build artifact path
    pub fn get_rust_build(
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

    /// Clean old cache entries based on age and size
    pub fn clean(&self, max_age_days: u64, max_size_mb: u64) -> LpmResult<CacheCleanResult> {
        use std::time::{Duration, SystemTime};

        let max_age = Duration::from_secs(max_age_days * 24 * 60 * 60);
        let max_size_bytes = max_size_mb * 1024 * 1024;
        let now = SystemTime::now();

        let mut result = CacheCleanResult {
            files_removed: 0,
            bytes_freed: 0,
        };

        // Clean rockspecs
        result += self.clean_directory(&self.rockspecs_dir(), &now, max_age, max_size_bytes)?;

        // Clean sources
        result += self.clean_directory(&self.sources_dir(), &now, max_age, max_size_bytes)?;

        // Clean Rust builds
        result += self.clean_directory(&self.rust_builds_dir(), &now, max_age, max_size_bytes)?;

        Ok(result)
    }

    /// Clean a directory based on age and total size
    fn clean_directory(
        &self,
        dir: &Path,
        now: &SystemTime,
        max_age: Duration,
        max_size_bytes: u64,
    ) -> LpmResult<CacheCleanResult> {
        use walkdir::WalkDir;

        if !dir.exists() {
            return Ok(CacheCleanResult::default());
        }

        let mut files: Vec<(PathBuf, SystemTime, u64)> = Vec::new();
        let mut total_size = 0u64;

        // Collect all files with metadata
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let size = metadata.len();
                        files.push((entry.path().to_path_buf(), modified, size));
                        total_size += size;
                    }
                }
            }
        }

        // Sort by modification time (oldest first)
        files.sort_by_key(|(_, modified, _)| *modified);

        let mut result = CacheCleanResult::default();

        // Remove files older than max_age
        for (path, modified, size) in &files {
            if let Ok(age) = now.duration_since(*modified) {
                if age > max_age {
                    if let Err(e) = fs::remove_file(path) {
                        eprintln!(
                            "Warning: Failed to remove old cache file {}: {}",
                            path.display(),
                            e
                        );
                    } else {
                        result.files_removed += 1;
                        result.bytes_freed += size;
                        total_size -= size;
                    }
                }
            }
        }

        // If still over size limit, remove oldest files
        if total_size > max_size_bytes {
            let target_size = max_size_bytes;
            for (path, _, size) in &files {
                if total_size <= target_size {
                    break;
                }
                if path.exists() {
                    if let Err(e) = fs::remove_file(path) {
                        eprintln!(
                            "Warning: Failed to remove cache file {}: {}",
                            path.display(),
                            e
                        );
                    } else {
                        result.files_removed += 1;
                        result.bytes_freed += size;
                        total_size -= size;
                    }
                }
            }
        }

        Ok(result)
    }
}

/// Result of cache cleaning operation
#[derive(Debug, Default)]
pub struct CacheCleanResult {
    pub files_removed: usize,
    pub bytes_freed: u64,
}

impl std::ops::AddAssign for CacheCleanResult {
    fn add_assign(&mut self, other: Self) {
        self.files_removed += other.files_removed;
        self.bytes_freed += other.bytes_freed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_init() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        cache.init().unwrap();

        assert!(cache.luarocks_dir().exists());
        assert!(cache.rockspecs_dir().exists());
        assert!(cache.sources_dir().exists());
        assert!(cache.rust_builds_dir().exists());
    }

    #[test]
    fn test_cache_read_write() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        cache.init().unwrap();

        let test_path = cache.rockspecs_dir().join("test.rockspec");
        let data = b"test data";

        cache.write(&test_path, data).unwrap();
        assert!(cache.exists(&test_path));

        let read_data = cache.read(&test_path).unwrap();
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_cache_new() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        assert_eq!(cache.root, temp.path());
    }

    #[test]
    fn test_cache_directory_paths() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        assert!(cache.luarocks_dir().ends_with("luarocks"));
        assert!(cache.rockspecs_dir().ends_with("rockspecs"));
        assert!(cache.sources_dir().ends_with("sources"));
        assert!(cache.rust_builds_dir().ends_with("rust-builds"));
    }

    #[test]
    fn test_cache_rockspec_path() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let path = cache.rockspec_path("test-package", "1.0.0");
        assert!(path.ends_with("test-package-1.0.0.rockspec"));
    }

    #[test]
    fn test_cache_source_path() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let url = "https://example.com/test.tar.gz";
        let path = cache.source_path(url);
        assert!(path.parent().unwrap().ends_with("sources"));
        // The extension might be extracted as "gz" not "tar.gz" depending on implementation
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        assert!(ext == "tar.gz" || ext == "gz" || path.to_string_lossy().contains("tar.gz"));
    }

    #[test]
    fn test_cache_checksum() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, b"test data").unwrap();

        let checksum = Cache::checksum(&test_file).unwrap();
        assert!(checksum.starts_with("blake3:")); // Now defaults to BLAKE3
        assert_eq!(checksum.len(), 71); // "blake3:" + 64 hex chars
    }

    #[test]
    fn test_cache_checksum_same_content() {
        let temp = TempDir::new().unwrap();
        let test_file1 = temp.path().join("test1.txt");
        let test_file2 = temp.path().join("test2.txt");
        std::fs::write(&test_file1, b"test data").unwrap();
        std::fs::write(&test_file2, b"test data").unwrap();

        let checksum1 = Cache::checksum(&test_file1).unwrap();
        let checksum2 = Cache::checksum(&test_file2).unwrap();
        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_cache_checksum_different_content() {
        let temp = TempDir::new().unwrap();
        let test_file1 = temp.path().join("test1.txt");
        let test_file2 = temp.path().join("test2.txt");
        std::fs::write(&test_file1, b"test data 1").unwrap();
        std::fs::write(&test_file2, b"test data 2").unwrap();

        let checksum1 = Cache::checksum(&test_file1).unwrap();
        let checksum2 = Cache::checksum(&test_file2).unwrap();
        assert_ne!(checksum1, checksum2);
    }

    #[test]
    fn test_cache_rust_build_path() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let path =
            cache.rust_build_path("test-package", "1.0.0", "5.4", "x86_64-unknown-linux-gnu");
        assert!(path.to_string_lossy().contains("test-package"));
        assert!(path.to_string_lossy().contains("1.0.0"));
        assert!(path.to_string_lossy().contains("5.4"));
        assert!(path.to_string_lossy().contains("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_cache_has_rust_build() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        assert!(!cache.has_rust_build("test-package", "1.0.0", "5.4", "x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_cache_read_nonexistent() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let nonexistent = temp.path().join("nonexistent.txt");
        let result = cache.read(&nonexistent);
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_exists() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let existing = temp.path().join("existing.txt");
        std::fs::write(&existing, b"test").unwrap();

        assert!(cache.exists(&existing));
        assert!(!cache.exists(&temp.path().join("nonexistent.txt")));
    }

    #[test]
    fn test_cache_write_read() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let test_file = temp.path().join("test.txt");
        let data = b"test data";
        cache.write(&test_file, data).unwrap();
        let read_data = cache.read(&test_file).unwrap();
        assert_eq!(read_data, data);
    }

    #[test]
    fn test_cache_url_hash() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let url1 = "https://example.com/test.tar.gz";
        let url2 = "https://example.com/test2.tar.gz";
        let hash1 = cache.source_path(url1);
        let hash2 = cache.source_path(url2);
        // Different URLs should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_cache_store_rust_build() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let artifact = temp.path().join("artifact.so");
        fs::write(&artifact, b"fake binary").unwrap();
        let result = cache.store_rust_build(
            "test-pkg",
            "1.0.0",
            "5.4",
            "x86_64-unknown-linux-gnu",
            &artifact,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_cache_clean_empty() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let result = cache.clean(30, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cache_get_rust_build_exists() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        // Store a build first
        let artifact = temp.path().join("artifact.so");
        fs::write(&artifact, b"fake binary").unwrap();
        cache
            .store_rust_build(
                "test-pkg",
                "1.0.0",
                "5.4",
                "x86_64-unknown-linux-gnu",
                &artifact,
            )
            .unwrap();

        // Should find it
        let result = cache.get_rust_build("test-pkg", "1.0.0", "5.4", "x86_64-unknown-linux-gnu");
        assert!(result.is_some());
    }

    #[test]
    fn test_cache_get_rust_build_not_exists() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let result =
            cache.get_rust_build("nonexistent", "1.0.0", "5.4", "x86_64-unknown-linux-gnu");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_clean_with_files() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        cache.init().unwrap();

        // Create some files in cache
        let test_file = cache.rockspecs_dir().join("test.rockspec");
        fs::write(&test_file, b"test content").unwrap();

        // Clean with large limits (should keep file)
        let result = cache.clean(30, 1000).unwrap();
        assert_eq!(result.files_removed, 0);
        assert!(test_file.exists());
    }

    #[test]
    fn test_cache_clean_result_add_assign() {
        let mut result1 = CacheCleanResult {
            files_removed: 5,
            bytes_freed: 1000,
        };
        let result2 = CacheCleanResult {
            files_removed: 3,
            bytes_freed: 500,
        };
        result1 += result2;
        assert_eq!(result1.files_removed, 8);
        assert_eq!(result1.bytes_freed, 1500);
    }

    #[test]
    fn test_cache_rust_build_path_windows_target() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let path = cache.rust_build_path("test-package", "1.0.0", "5.4", "x86_64-pc-windows-msvc");
        assert!(path.to_string_lossy().contains("dll"));
    }

    #[test]
    fn test_cache_rust_build_path_macos_target() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let path = cache.rust_build_path("test-package", "1.0.0", "5.4", "x86_64-apple-darwin");
        assert!(path.to_string_lossy().contains("dylib"));
    }

    #[test]
    fn test_cache_rust_build_path_linux_target() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let path =
            cache.rust_build_path("test-package", "1.0.0", "5.4", "x86_64-unknown-linux-gnu");
        assert!(path.to_string_lossy().contains(".so"));
    }

    #[test]
    fn test_cache_write_creates_parent_dirs() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let nested_path = temp.path().join("deep").join("nested").join("file.txt");
        cache.write(&nested_path, b"test").unwrap();
        assert!(nested_path.exists());
    }

    #[test]
    fn test_cache_source_path_different_extensions() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        let path1 = cache.source_path("https://example.com/file.zip");
        let path2 = cache.source_path("https://example.com/file.tar.gz");

        // Both should be in sources dir
        assert!(path1.parent().unwrap().ends_with("sources"));
        assert!(path2.parent().unwrap().ends_with("sources"));
    }

    #[test]
    fn test_cache_store_and_retrieve_rust_build() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();

        // Create artifact
        let artifact = temp.path().join("lib.so");
        fs::write(&artifact, b"binary content").unwrap();

        // Store it
        let cached_path = cache
            .store_rust_build(
                "my-pkg",
                "2.0.0",
                "5.3",
                "x86_64-unknown-linux-gnu",
                &artifact,
            )
            .unwrap();

        assert!(cached_path.exists());
        assert!(cache.has_rust_build("my-pkg", "2.0.0", "5.3", "x86_64-unknown-linux-gnu"));

        // Retrieve it
        let retrieved = cache
            .get_rust_build("my-pkg", "2.0.0", "5.3", "x86_64-unknown-linux-gnu")
            .unwrap();
        assert_eq!(cached_path, retrieved);
    }

    #[test]
    fn test_checksum_with_blake3() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"Hello, BLAKE3!").unwrap();

        let checksum =
            Cache::checksum_with_algorithm(&test_file, ChecksumAlgorithm::Blake3).unwrap();
        assert!(checksum.starts_with("blake3:"));
        assert_eq!(checksum.len(), 71); // "blake3:" + 64 hex chars
    }

    #[test]
    fn test_checksum_with_sha256() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"Hello, SHA-256!").unwrap();

        let checksum =
            Cache::checksum_with_algorithm(&test_file, ChecksumAlgorithm::Sha256).unwrap();
        assert!(checksum.starts_with("sha256:"));
        assert_eq!(checksum.len(), 71); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_checksum_defaults_to_blake3() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"Test content").unwrap();

        let checksum = Cache::checksum(&test_file).unwrap();
        assert!(checksum.starts_with("blake3:"));
    }

    #[test]
    fn test_verify_checksum_blake3() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"Test content").unwrap();

        let checksum =
            Cache::checksum_with_algorithm(&test_file, ChecksumAlgorithm::Blake3).unwrap();
        assert!(Cache::verify_checksum(&test_file, &checksum).unwrap());
    }

    #[test]
    fn test_verify_checksum_sha256() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"Test content").unwrap();

        let checksum =
            Cache::checksum_with_algorithm(&test_file, ChecksumAlgorithm::Sha256).unwrap();
        assert!(Cache::verify_checksum(&test_file, &checksum).unwrap());
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"Test content").unwrap();

        let wrong_checksum =
            "blake3:0000000000000000000000000000000000000000000000000000000000000000";
        assert!(!Cache::verify_checksum(&test_file, wrong_checksum).unwrap());
    }

    #[test]
    fn test_checksum_algorithm_from_checksum() {
        assert_eq!(
            ChecksumAlgorithm::from_checksum("blake3:abc123"),
            ChecksumAlgorithm::Blake3
        );
        assert_eq!(
            ChecksumAlgorithm::from_checksum("sha256:def456"),
            ChecksumAlgorithm::Sha256
        );
        assert_eq!(
            ChecksumAlgorithm::from_checksum("unprefixed"),
            ChecksumAlgorithm::Blake3
        );
    }

    #[test]
    fn test_checksum_algorithm_default() {
        assert_eq!(ChecksumAlgorithm::default(), ChecksumAlgorithm::Blake3);
    }
}
