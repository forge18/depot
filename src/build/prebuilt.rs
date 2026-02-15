use crate::build::targets::Target;
use crate::cache::Cache;
use crate::core::path::cache_dir;
use crate::core::{DepotError, DepotResult};
use crate::lua_version::detector::LuaVersion;
use std::fs;
use std::path::PathBuf;

/// Manages pre-built binary downloads for Rust-compiled Lua native modules
///
/// These are pre-compiled dynamic libraries (.so/.dylib/.dll) that were built
/// from Rust code and are part of Lua module packages. NOT standalone Rust libraries.
pub struct PrebuiltBinaryManager {
    cache: Cache,
}

impl PrebuiltBinaryManager {
    /// Create a new pre-built binary manager
    pub fn new() -> DepotResult<Self> {
        let cache = Cache::new(cache_dir()?)?;
        Ok(Self { cache })
    }

    /// Check if a pre-built native module binary is available for a package
    ///
    /// These are compiled Rust code as dynamic libraries (.so/.dylib/.dll)
    /// that are part of Lua module packages.
    ///
    /// This checks:
    /// 1. Local cache (already downloaded)
    /// 2. Package manifest for binary URLs (future: CDN/registry)
    pub fn has_prebuilt(
        &self,
        package: &str,
        version: &str,
        lua_version: &LuaVersion,
        target: &Target,
    ) -> bool {
        let lua_version_str = lua_version.major_minor();
        self.cache
            .has_rust_build(package, version, &lua_version_str, &target.triple)
    }

    /// Get the path to a pre-built binary if available
    pub fn get_prebuilt(
        &self,
        package: &str,
        version: &str,
        lua_version: &LuaVersion,
        target: &Target,
    ) -> Option<PathBuf> {
        let lua_version_str = lua_version.major_minor();
        self.cache
            .get_rust_build(package, version, &lua_version_str, &target.triple)
    }

    /// Download a pre-built native module binary from a URL
    ///
    /// Downloads a compiled Rust dynamic library (.so/.dylib/.dll) that is
    /// part of a Lua module package and stores it in the cache.
    pub async fn download_prebuilt(
        &self,
        package: &str,
        version: &str,
        lua_version: &LuaVersion,
        target: &Target,
        url: &str,
    ) -> DepotResult<PathBuf> {
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        let lua_version_str = lua_version.major_minor();
        let cache_path =
            self.cache
                .rust_build_path(package, version, &lua_version_str, &target.triple);

        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        eprintln!("Downloading pre-built binary from {}...", url);

        // Download the binary
        let response = reqwest::get(url).await.map_err(|e| {
            DepotError::Package(format!("Failed to download pre-built binary: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(DepotError::Package(format!(
                "Failed to download pre-built binary: HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| DepotError::Package(format!("Failed to read binary data: {}", e)))?;

        // Write to cache
        let mut file = File::create(&cache_path)
            .await
            .map_err(|e| DepotError::Cache(format!("Failed to create cache file: {}", e)))?;

        file.write_all(&bytes)
            .await
            .map_err(|e| DepotError::Cache(format!("Failed to write binary to cache: {}", e)))?;

        file.sync_all()
            .await
            .map_err(|e| DepotError::Cache(format!("Failed to sync cache file: {}", e)))?;

        eprintln!("✓ Downloaded pre-built binary: {}", cache_path.display());

        Ok(cache_path)
    }

    /// Find binary URL from a package's binary_urls table
    ///
    /// Looks for a binary URL matching the current Lua version and target.
    /// Format: `binary_urls: { "5.4-x86_64-unknown-linux-gnu": "https://..." }`
    pub fn find_binary_url(
        binary_urls: &std::collections::HashMap<String, String>,
        target: &Target,
        lua_version: &LuaVersion,
    ) -> Option<String> {
        // The key format is: "{lua_version}-{target_triple}"
        let key = format!("{}-{}", lua_version.major_minor(), target.triple);
        binary_urls.get(&key).cloned()
    }

    /// Try to get or download a pre-built binary
    ///
    /// Returns the path to the binary if available, or None if not available
    pub async fn get_or_download(
        &self,
        package: &str,
        version: &str,
        lua_version: &LuaVersion,
        target: &Target,
        binary_url: Option<&str>,
    ) -> DepotResult<Option<PathBuf>> {
        // First, check if we already have it cached
        if let Some(cached) = self.get_prebuilt(package, version, lua_version, target) {
            return Ok(Some(cached));
        }

        // If a binary URL is provided, try to download it
        if let Some(url) = binary_url {
            match self
                .download_prebuilt(package, version, lua_version, target, url)
                .await
            {
                Ok(path) => Ok(Some(path)),
                Err(e) => {
                    eprintln!("Warning: Failed to download pre-built binary: {}", e);
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_prebuilt_manager() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let manager = PrebuiltBinaryManager { cache };

        let lua_version = LuaVersion::new(5, 4, 0);
        let target = Target::default_target();

        // Should not have pre-built binary for non-existent package
        assert!(!manager.has_prebuilt("test-package", "1.0.0", &lua_version, &target));
    }

    #[test]
    fn test_find_binary_url() {
        let mut binary_urls = std::collections::HashMap::new();
        binary_urls.insert(
            "5.4-x86_64-unknown-linux-gnu".to_string(),
            "https://example.com/linux.so".to_string(),
        );
        binary_urls.insert(
            "5.4-x86_64-apple-darwin".to_string(),
            "https://example.com/darwin.dylib".to_string(),
        );

        let target = Target::new("x86_64-unknown-linux-gnu").unwrap();
        let lua_version = LuaVersion::new(5, 4, 0);

        let url = PrebuiltBinaryManager::find_binary_url(&binary_urls, &target, &lua_version);
        assert_eq!(url, Some("https://example.com/linux.so".to_string()));
    }

    #[test]
    fn test_find_binary_url_not_found() {
        let binary_urls = std::collections::HashMap::new();

        let target = Target::new("x86_64-unknown-linux-gnu").unwrap();
        let lua_version = LuaVersion::new(5, 4, 0);

        let url = PrebuiltBinaryManager::find_binary_url(&binary_urls, &target, &lua_version);
        assert!(url.is_none());
    }

    #[test]
    fn test_find_binary_url_different_lua_version() {
        let mut binary_urls = std::collections::HashMap::new();
        binary_urls.insert(
            "5.4-x86_64-unknown-linux-gnu".to_string(),
            "https://example.com/lua54.so".to_string(),
        );
        binary_urls.insert(
            "5.3-x86_64-unknown-linux-gnu".to_string(),
            "https://example.com/lua53.so".to_string(),
        );

        let target = Target::new("x86_64-unknown-linux-gnu").unwrap();
        let lua53 = LuaVersion::new(5, 3, 0);
        let lua54 = LuaVersion::new(5, 4, 0);

        let url53 = PrebuiltBinaryManager::find_binary_url(&binary_urls, &target, &lua53);
        assert_eq!(url53, Some("https://example.com/lua53.so".to_string()));

        let url54 = PrebuiltBinaryManager::find_binary_url(&binary_urls, &target, &lua54);
        assert_eq!(url54, Some("https://example.com/lua54.so".to_string()));
    }

    #[test]
    fn test_has_prebuilt_false() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let manager = PrebuiltBinaryManager { cache };

        let lua_version = LuaVersion::new(5, 4, 0);
        let target = Target::default_target();

        assert!(!manager.has_prebuilt("nonexistent", "1.0.0", &lua_version, &target));
    }

    #[test]
    fn test_get_prebuilt_none() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let manager = PrebuiltBinaryManager { cache };

        let lua_version = LuaVersion::new(5, 4, 0);
        let target = Target::default_target();

        let path = manager.get_prebuilt("nonexistent", "1.0.0", &lua_version, &target);
        assert!(path.is_none());
    }

    #[tokio::test]
    async fn test_get_or_download_no_url() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let manager = PrebuiltBinaryManager { cache };

        let lua_version = LuaVersion::new(5, 4, 0);
        let target = Target::default_target();

        let result = manager
            .get_or_download("test-pkg", "1.0.0", &lua_version, &target, None)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_prebuilt_manager_new() {
        // This test may fail if cache_dir() fails, which is expected in test environment
        let result = PrebuiltBinaryManager::new();
        // Just test that it doesn't panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_get_or_download_with_cached() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let manager = PrebuiltBinaryManager { cache };

        let lua_version = LuaVersion::new(5, 4, 0);
        let target = Target::default_target();

        // Create a fake cached binary
        let lua_version_str = lua_version.major_minor();
        let cache_path =
            manager
                .cache
                .rust_build_path("test-pkg", "1.0.0", &lua_version_str, &target.triple);
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&cache_path, b"fake binary").unwrap();

        // Should return cached path even with URL provided
        let result = manager
            .get_or_download(
                "test-pkg",
                "1.0.0",
                &lua_version,
                &target,
                Some("https://example.com/binary.so"),
            )
            .await;
        assert!(result.is_ok());
        let path_opt = result.unwrap();
        assert!(path_opt.is_some());
        assert_eq!(path_opt.unwrap(), cache_path);
    }

    #[test]
    fn test_find_binary_url_with_multiple_targets() {
        let mut binary_urls = std::collections::HashMap::new();
        binary_urls.insert(
            "5.4-x86_64-unknown-linux-gnu".to_string(),
            "https://example.com/linux.so".to_string(),
        );
        binary_urls.insert(
            "5.4-x86_64-apple-darwin".to_string(),
            "https://example.com/darwin.dylib".to_string(),
        );
        binary_urls.insert(
            "5.4-aarch64-apple-darwin".to_string(),
            "https://example.com/darwin-arm.dylib".to_string(),
        );

        let target_darwin = Target::new("x86_64-apple-darwin").unwrap();
        let lua_version = LuaVersion::new(5, 4, 0);

        let url =
            PrebuiltBinaryManager::find_binary_url(&binary_urls, &target_darwin, &lua_version);
        assert_eq!(url, Some("https://example.com/darwin.dylib".to_string()));
    }
}
