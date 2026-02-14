//! Lockfile builder - Takes a manifest, resolves dependencies, downloads tarballs, and builds a lockfile

use crate::core::path::packages_metadata_dir;
use crate::core::{DepotError, DepotResult};
use crate::di::traits::{CacheProvider, GitHubProvider};
use crate::package::downloader::{DownloadTask, ParallelDownloader};
use crate::package::lockfile::{LockedPackage, Lockfile};
use crate::package::manifest::PackageManifest;
use crate::package::metadata::PackageMetadata;
use crate::resolver::{DependencyResolver, ResolvedPackage};
use chrono::Utc;
use depot_core::package::manifest::DependencySpec;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Builds a lockfile from a manifest by resolving dependencies, downloading tarballs, and calculating checksums
pub struct LockfileBuilder {
    project_root: PathBuf,
    cache: Arc<dyn CacheProvider>,
    github: Arc<dyn GitHubProvider>,
    fallback_chain: Vec<String>,
}

impl LockfileBuilder {
    /// Create a new lockfile builder
    pub fn new(
        project_root: &Path,
        cache: Arc<dyn CacheProvider>,
        github: Arc<dyn GitHubProvider>,
        fallback_chain: Vec<String>,
    ) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            cache,
            github,
            fallback_chain,
        }
    }

    /// Build a complete lockfile from a manifest
    ///
    /// Steps:
    /// 1. Validate that each package has a valid metadata file
    /// 2. Take the dependencies from .depot (if it exists)
    /// 3. Resolve dependencies if they exist
    /// 4. Download the tarballs
    /// 5. Calculate checksums
    /// 6. Build a Lockfile
    /// 7. Set the installed-on (if the first install) and updated-on props in the metadata file
    pub async fn build(&self, manifest: &PackageManifest) -> DepotResult<Lockfile> {
        println!("Building lockfile from manifest...");

        // Step 1: Validate that each package has a valid metadata file
        self.validate_metadata_files(manifest)?;

        // Step 2: Take the dependencies from .depot (if it exists) - already in manifest
        let dependencies = &manifest.dependencies;

        if dependencies.is_empty() {
            println!("  No dependencies found");
            return Ok(Lockfile {
                version: 2,
                generated_at: Utc::now(),
                packages: HashMap::new(),
            });
        }

        println!("  Found {} dependencies", dependencies.len());

        // Convert HashMap<String, String> to HashMap<String, DependencySpec>
        let dep_specs: HashMap<String, DependencySpec> = dependencies
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    DependencySpec {
                        version: Some(v.clone()),
                        repository: None,
                    },
                )
            })
            .collect();

        // Step 3: Resolve dependencies if they exist
        println!("  Resolving dependencies...");
        let resolved = self.resolve_dependencies(&dep_specs).await?;
        println!(
            "  Resolved {} packages (including transitive)",
            resolved.len()
        );

        // Step 4: Download the tarballs
        println!("  Downloading tarballs...");
        let download_results = self.download_tarballs(&resolved).await?;

        // Step 5: Calculate checksums
        println!("  Calculating checksums...");
        let mut locked_packages = HashMap::new();

        for (repo, resolved_pkg) in &resolved {
            // Find the corresponding download result
            let download_result = download_results
                .iter()
                .find(|r| r.repository == *repo)
                .ok_or_else(|| {
                    DepotError::Package(format!("Download result not found for {}", repo))
                })?;

            if let Some(ref error) = download_result.error {
                return Err(DepotError::Package(format!(
                    "Failed to download {}: {}",
                    repo, error
                )));
            }

            // Calculate checksum
            let checksum = self.cache.checksum(&download_result.tarball_path)?;
            let size = fs::metadata(&download_result.tarball_path)?.len();

            // Convert dependencies to simple map
            let dep_map: HashMap<String, String> = resolved_pkg
                .dependencies
                .iter()
                .map(|(k, v)| {
                    let version_str = v.version.clone().unwrap_or_else(|| "latest".to_string());
                    (k.clone(), version_str)
                })
                .collect();

            // Create locked package
            let locked_pkg = LockedPackage {
                version: resolved_pkg.version.clone(),
                repository: repo.clone(),
                ref_type: format!("{:?}", resolved_pkg.resolved.ref_type),
                ref_value: resolved_pkg.resolved.ref_value.clone(),
                commit_sha: resolved_pkg.resolved.commit_sha.clone(),
                tarball_url: resolved_pkg.resolved.tarball_url.clone(),
                checksum,
                size,
                dependencies: dep_map,
                build: None,
                native_code: None,
            };

            locked_packages.insert(repo.clone(), locked_pkg);
        }

        // Step 6: Build a Lockfile
        let lockfile = Lockfile {
            version: 2,
            generated_at: Utc::now(),
            packages: locked_packages,
        };

        // Step 7: Set the installed-on (if the first install) and updated-on props in the metadata file
        self.update_metadata_timestamps(&resolved)?;

        println!("  ✓ Lockfile built successfully");
        Ok(lockfile)
    }

    /// Step 1: Validate that each package has a valid metadata file
    fn validate_metadata_files(&self, manifest: &PackageManifest) -> DepotResult<()> {
        let packages_dir = packages_metadata_dir(&self.project_root);

        for name in manifest.dependencies.keys() {
            let metadata_path = packages_dir.join(name).join(".metadata");

            // If metadata file exists, validate it
            if metadata_path.exists() {
                PackageMetadata::load(&metadata_path)?;
            }
            // If it doesn't exist, that's okay - it means it's a new package
        }

        Ok(())
    }

    /// Step 3: Resolve dependencies if they exist
    async fn resolve_dependencies(
        &self,
        dependencies: &HashMap<String, DependencySpec>,
    ) -> DepotResult<HashMap<String, ResolvedPackage>> {
        let resolver =
            DependencyResolver::new(Arc::clone(&self.github), self.fallback_chain.clone());

        resolver.resolve(dependencies).await
    }

    /// Step 4: Download the tarballs
    async fn download_tarballs(
        &self,
        resolved: &HashMap<String, ResolvedPackage>,
    ) -> DepotResult<Vec<crate::package::downloader::DownloadResult>> {
        let tasks: Vec<DownloadTask> = resolved
            .iter()
            .map(|(repo, pkg)| DownloadTask {
                repository: repo.clone(),
                version: Some(pkg.version.clone()),
                resolved: Some(pkg.resolved.clone()),
            })
            .collect();

        let downloader = ParallelDownloader::new(
            Arc::clone(&self.github),
            self.fallback_chain.clone(),
            Some(10),
        );

        downloader.download_with_progress(tasks).await
    }

    /// Step 7: Set the installed-on (if the first install) and updated-on props in the metadata file
    fn update_metadata_timestamps(
        &self,
        resolved: &HashMap<String, ResolvedPackage>,
    ) -> DepotResult<()> {
        let packages_dir = packages_metadata_dir(&self.project_root);

        for (repo, pkg) in resolved {
            let metadata_path = packages_dir.join(repo).join(".metadata");

            if metadata_path.exists() {
                // Update existing metadata
                let mut metadata = PackageMetadata::load(&metadata_path)?;
                metadata.touch();
                metadata.save(&metadata_path)?;
            } else {
                // Create new metadata (this is the first install)
                let metadata = PackageMetadata::new(
                    repo.clone(),
                    pkg.version.clone(),
                    repo.clone(),
                    format!("{:?}", pkg.resolved.ref_type),
                    pkg.resolved.ref_value.clone(),
                    pkg.resolved.commit_sha.clone(),
                );
                metadata.save(&metadata_path)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::di::mocks::{MockCacheProvider, MockGitHubProvider};
    use crate::github::types::{RefType, ResolvedVersion};
    use crate::github::GitHubRelease;
    use tempfile::TempDir;

    #[test]
    fn test_new_builder() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let builder = LockfileBuilder::new(temp.path(), cache, github, fallback);
        assert_eq!(builder.project_root, temp.path());
    }

    #[tokio::test]
    async fn test_build_empty_manifest() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let builder = LockfileBuilder::new(temp.path(), cache, github, fallback);
        let manifest = PackageManifest::default("test".to_string());

        let lockfile = builder.build(&manifest).await.unwrap();
        assert_eq!(lockfile.version, 2);
        assert!(lockfile.packages.is_empty());
    }

    #[test]
    fn test_validate_metadata_files_nonexistent() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let builder = LockfileBuilder::new(temp.path(), cache, github, fallback);
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("owner/repo".to_string(), "1.0.0".to_string());

        // Should not fail if metadata doesn't exist (new package)
        let result = builder.validate_metadata_files(&manifest);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_metadata_files_invalid() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        // Create invalid metadata file
        let packages_dir = packages_metadata_dir(temp.path());
        fs::create_dir_all(packages_dir.join("owner/repo")).unwrap();
        fs::write(packages_dir.join("owner/repo/.metadata"), "invalid yaml").unwrap();

        let builder = LockfileBuilder::new(temp.path(), cache, github, fallback);
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("owner/repo".to_string(), "1.0.0".to_string());

        let result = builder.validate_metadata_files(&manifest);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_build_with_dependencies() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());

        // Setup mock data
        github.add_release(
            "owner",
            "repo",
            GitHubRelease {
                tag_name: "v1.0.0".to_string(),
                name: Some("Release v1.0.0".to_string()),
                tarball_url: "https://api.github.com/repos/owner/repo/tarball/v1.0.0".to_string(),
                zipball_url: "https://api.github.com/repos/owner/repo/zipball/v1.0.0".to_string(),
                prerelease: false,
                draft: false,
                assets: Vec::new(),
                body: Some("Test release".to_string()),
                published_at: Some("2024-01-01T00:00:00Z".to_string()),
            },
        );
        // Set up mock tarball - cache will return this path when asked to download
        let tarball_url = "https://api.github.com/repos/owner/repo/tarball/v1.0.0";
        let tarball_path = cache.source_path(tarball_url);

        // Add the file to the mock cache
        cache.add_file(tarball_path.clone(), b"test tarball content".to_vec());

        // Create the actual file on disk for checksum calculation
        std::fs::create_dir_all(tarball_path.parent().unwrap()).unwrap();
        std::fs::write(&tarball_path, b"test tarball content").unwrap();

        github.add_tarball("owner", "repo", "v1.0.0", tarball_path);

        let fallback = vec!["release".to_string()];
        let builder = LockfileBuilder::new(temp.path(), cache, github, fallback);

        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("owner/repo".to_string(), "v1.0.0".to_string());

        let lockfile = builder.build(&manifest).await.unwrap();
        assert_eq!(lockfile.version, 2);
        assert_eq!(lockfile.packages.len(), 1);
        assert!(lockfile.packages.contains_key("owner/repo"));
    }

    #[test]
    fn test_update_metadata_timestamps() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let builder = LockfileBuilder::new(temp.path(), cache, github, fallback);

        let mut resolved = HashMap::new();
        resolved.insert(
            "owner/repo".to_string(),
            ResolvedPackage {
                repository: "owner/repo".to_string(),
                version: "1.0.0".to_string(),
                resolved: ResolvedVersion {
                    ref_type: RefType::Release,
                    ref_value: "v1.0.0".to_string(),
                    commit_sha: "abc123".to_string(),
                    tarball_url: "https://api.github.com/repos/owner/repo/tarball/v1.0.0"
                        .to_string(),
                },
                dependencies: HashMap::new(),
            },
        );

        let result = builder.update_metadata_timestamps(&resolved);
        assert!(result.is_ok());

        // Check that metadata was created
        let packages_dir = packages_metadata_dir(temp.path());
        let metadata_path = packages_dir.join("owner/repo/.metadata");
        assert!(metadata_path.exists());

        let metadata = PackageMetadata::load(&metadata_path).unwrap();
        assert_eq!(metadata.package_name, "owner/repo");
        assert!(metadata.is_fresh_install());
    }
}
