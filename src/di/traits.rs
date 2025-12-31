//! Trait definitions for dependency injection

use crate::core::LpmResult;
use crate::luarocks::manifest::Manifest;
use crate::luarocks::rockspec::Rockspec;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Trait for configuration access
///
/// Provides read-only access to application configuration.
/// Implementations should be thread-safe (Send + Sync).
pub trait ConfigProvider: Send + Sync {
    /// Get the LuaRocks manifest URL
    fn luarocks_manifest_url(&self) -> &str;

    /// Get the cache directory path
    fn cache_dir(&self) -> LpmResult<PathBuf>;

    /// Check if checksum verification is enabled
    fn verify_checksums(&self) -> bool;

    /// Check if diffs should be shown on update
    fn show_diffs_on_update(&self) -> bool;

    /// Get the resolution strategy (e.g., "highest", "lowest")
    fn resolution_strategy(&self) -> &str;

    /// Get the checksum algorithm (e.g., "blake3", "sha256")
    fn checksum_algorithm(&self) -> &str;

    /// Check if strict conflict detection is enabled
    fn strict_conflicts(&self) -> bool;

    /// Get the Lua binary source URL (optional)
    fn lua_binary_source_url(&self) -> Option<&str>;

    /// Get the list of supported Lua versions (optional)
    fn supported_lua_versions(&self) -> Option<&Vec<String>>;
}

/// Trait for cache operations
///
/// Provides access to the package cache for storing and retrieving
/// rockspecs, source archives, and build artifacts.
pub trait CacheProvider: Send + Sync {
    /// Get the cache path for a rockspec file
    fn rockspec_path(&self, package: &str, version: &str) -> PathBuf;

    /// Get the cache path for a source archive
    fn source_path(&self, url: &str) -> PathBuf;

    /// Check if a file exists in the cache
    fn exists(&self, path: &Path) -> bool;

    /// Read a file from the cache
    fn read(&self, path: &Path) -> LpmResult<Vec<u8>>;

    /// Write a file to the cache
    fn write(&self, path: &Path, data: &[u8]) -> LpmResult<()>;

    /// Calculate the checksum of a file
    fn checksum(&self, path: &Path) -> LpmResult<String>;

    /// Verify the checksum of a file
    fn verify_checksum(&self, path: &Path, expected: &str) -> LpmResult<bool>;

    /// Get the cache path for a Rust build artifact
    fn rust_build_path(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
    ) -> PathBuf;

    /// Check if a Rust build artifact exists in cache
    fn has_rust_build(&self, package: &str, version: &str, lua_version: &str, target: &str)
        -> bool;

    /// Store a Rust build artifact in cache
    fn store_rust_build(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
        artifact_path: &Path,
    ) -> LpmResult<PathBuf>;

    /// Get a Rust build artifact from cache
    fn get_rust_build(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
    ) -> Option<PathBuf>;
}

/// Trait for LuaRocks package client operations
///
/// Provides async methods for interacting with the LuaRocks repository,
/// including downloading rockspecs and source packages.
#[async_trait]
pub trait PackageClient: Send + Sync {
    /// Fetch the LuaRocks manifest
    async fn fetch_manifest(&self) -> LpmResult<Manifest>;

    /// Download a rockspec file from a URL
    async fn download_rockspec(&self, url: &str) -> LpmResult<String>;

    /// Parse a rockspec from its Lua content
    fn parse_rockspec(&self, content: &str) -> LpmResult<Rockspec>;

    /// Download a source package from a URL
    async fn download_source(&self, url: &str) -> LpmResult<PathBuf>;
}

/// Trait for package search and discovery
///
/// Provides methods for searching packages and constructing
/// rockspec URLs.
#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Get the latest version of a package
    async fn get_latest_version(&self, package_name: &str) -> LpmResult<String>;

    /// Construct a rockspec URL for a package
    fn get_rockspec_url(&self, package_name: &str, version: &str, manifest: Option<&str>)
        -> String;

    /// Verify that a rockspec URL is accessible
    async fn verify_rockspec_url(&self, url: &str) -> LpmResult<()>;
}
