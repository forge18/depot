//! Trait definitions for dependency injection

use crate::core::DepotResult;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Trait for configuration access
///
/// Provides read-only access to application configuration.
/// Implementations should be thread-safe (Send + Sync).
pub trait ConfigProvider: Send + Sync {
    /// Get the cache directory path
    fn cache_dir(&self) -> DepotResult<PathBuf>;

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

    /// Get the GitHub API URL
    fn github_api_url(&self) -> &str;

    /// Get the GitHub token (checks environment variable first, then config)
    fn github_token(&self) -> Option<String>;

    /// Get the GitHub fallback chain for version resolution
    fn github_fallback_chain(&self) -> &[String];

    /// Check if strict native code warnings are enabled
    fn strict_native_code(&self) -> bool;
}

/// Trait for cache operations
///
/// Provides access to the package cache for storing and retrieving
/// package metadata, source archives, and build artifacts.
pub trait CacheProvider: Send + Sync {
    /// Get the cache path for a package metadata file
    fn package_metadata_path(&self, package: &str, version: &str) -> PathBuf;

    /// Get the cache path for a source archive
    fn source_path(&self, url: &str) -> PathBuf;

    /// Check if a file exists in the cache
    fn exists(&self, path: &Path) -> bool;

    /// Read a file from the cache
    fn read(&self, path: &Path) -> DepotResult<Vec<u8>>;

    /// Write a file to the cache
    fn write(&self, path: &Path, data: &[u8]) -> DepotResult<()>;

    /// Calculate the checksum of a file
    fn checksum(&self, path: &Path) -> DepotResult<String>;

    /// Verify the checksum of a file
    fn verify_checksum(&self, path: &Path, expected: &str) -> DepotResult<bool>;

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
    ) -> DepotResult<PathBuf>;

    /// Get a Rust build artifact from cache
    fn get_rust_build(
        &self,
        package: &str,
        version: &str,
        lua_version: &str,
        target: &str,
    ) -> Option<PathBuf>;
}

/// Trait for GitHub API operations
///
/// Provides async methods for interacting with GitHub to fetch releases,
/// tags, branches, and download source tarballs.
#[async_trait]
pub trait GitHubProvider: Send + Sync {
    /// Get all releases for a repository
    async fn get_releases(
        &self,
        owner: &str,
        repo: &str,
    ) -> DepotResult<Vec<crate::github::GitHubRelease>>;

    /// Get the latest release for a repository
    async fn get_latest_release(
        &self,
        owner: &str,
        repo: &str,
    ) -> DepotResult<crate::github::GitHubRelease>;

    /// Get all tags for a repository
    async fn get_tags(&self, owner: &str, repo: &str)
        -> DepotResult<Vec<crate::github::GitHubTag>>;

    /// Get the default branch for a repository
    async fn get_default_branch(&self, owner: &str, repo: &str) -> DepotResult<String>;

    /// Get file content from a repository at a specific ref
    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        ref_: &str,
    ) -> DepotResult<String>;

    /// Download a tarball for a specific ref
    async fn download_tarball(&self, owner: &str, repo: &str, ref_: &str) -> DepotResult<PathBuf>;

    /// Resolve a version using the fallback chain
    async fn resolve_version(
        &self,
        owner: &str,
        repo: &str,
        version_spec: Option<&str>,
        fallback_chain: &[String],
    ) -> DepotResult<crate::github::ResolvedVersion>;
}
