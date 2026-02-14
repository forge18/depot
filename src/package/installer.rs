//! GitHub-based package installer

use crate::core::path::{depot_metadata_dir, ensure_dir, lua_modules_dir, packages_metadata_dir};
use crate::core::{DepotError, DepotResult};
use crate::di::traits::{CacheProvider, GitHubProvider};
use crate::github::types::ResolvedVersion;
use crate::package::extractor::PackageExtractor;
use crate::package::lockfile::{LockedPackage, Lockfile};
use crate::package::manifest::PackageManifest;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

/// Install packages from GitHub to lua_modules/
pub struct PackageInstaller {
    project_root: PathBuf,
    lua_modules: PathBuf,
    metadata_dir: PathBuf,
    packages_dir: PathBuf,
    cache: Arc<dyn CacheProvider>,
    github: Arc<dyn GitHubProvider>,
    extractor: PackageExtractor,
    fallback_chain: Vec<String>,
}

impl PackageInstaller {
    /// Create a new installer with injected dependencies
    pub fn new(
        project_root: &Path,
        cache: Arc<dyn CacheProvider>,
        github: Arc<dyn GitHubProvider>,
        fallback_chain: Vec<String>,
    ) -> DepotResult<Self> {
        let lua_modules = lua_modules_dir(project_root);
        let metadata_dir = depot_metadata_dir(project_root);
        let packages_dir = packages_metadata_dir(project_root);
        let extractor = PackageExtractor::new(lua_modules.clone());

        Ok(Self {
            project_root: project_root.to_path_buf(),
            lua_modules,
            metadata_dir,
            packages_dir,
            cache,
            github,
            extractor,
            fallback_chain,
        })
    }

    /// Initialize the directory structure
    pub fn init(&self) -> DepotResult<()> {
        ensure_dir(&self.lua_modules)?;
        ensure_dir(&self.metadata_dir)?;
        ensure_dir(&self.packages_dir)?;
        Ok(())
    }

    /// Install a package from GitHub
    ///
    /// Format: owner/repo[@version]
    pub async fn install_package(
        &self,
        repository: &str,
        version: Option<&str>,
    ) -> DepotResult<PathBuf> {
        println!("Installing {}", repository);

        // Parse owner/repo
        let parts: Vec<&str> = repository.split('/').collect();
        if parts.len() != 2 {
            return Err(DepotError::Config(format!(
                "Invalid repository format '{}'. Expected 'owner/repo'",
                repository
            )));
        }
        let owner = parts[0];
        let repo = parts[1];

        // Step 1: Resolve version using GitHub API
        println!("  Resolving version...");
        let resolved = self
            .github
            .resolve_version(owner, repo, version, &self.fallback_chain)
            .await?;

        println!(
            "  Resolved to: {} ({})",
            resolved.ref_value, resolved.ref_type
        );

        // Step 2: Download tarball
        println!("  Downloading...");
        let tarball_path = self
            .github
            .download_tarball(owner, repo, &resolved.ref_value)
            .await?;

        // Step 3: Verify checksum if lockfile exists
        if let Some(lockfile) = Lockfile::load(&self.project_root)? {
            if let Some(locked_pkg) = lockfile.get_package(repository) {
                println!("  Verifying checksum...");
                let actual = self.cache.checksum(&tarball_path)?;
                if actual != locked_pkg.checksum {
                    return Err(DepotError::Package(format!(
                        "Checksum mismatch for {}. Expected {}, got {}",
                        repository, locked_pkg.checksum, actual
                    )));
                }
                println!("  ✓ Checksum verified");
            }
        }

        // Step 4: Extract tarball
        println!("  Extracting...");
        let extracted_path = self.extractor.extract(&tarball_path)?;

        // Step 5: Try to read package.yaml for build instructions
        let package_manifest = self.read_package_manifest(&extracted_path).ok();

        // Step 6: Install files
        println!("  Installing...");
        let package_name = format!("{}/{}", owner, repo);
        self.install_from_extracted(&extracted_path, &package_name, package_manifest.as_ref())?;

        // Step 7: Calculate checksum and size for lockfile
        let checksum = self.cache.checksum(&tarball_path)?;
        let _size = fs::metadata(&tarball_path)?.len();

        println!("  ✓ Installed {} (checksum: {})", package_name, checksum);

        Ok(self.lua_modules.join(&package_name))
    }

    /// Read package.yaml from extracted package
    fn read_package_manifest(&self, extracted_path: &Path) -> DepotResult<PackageManifest> {
        let filenames = vec!["package.yaml", "package.yml", ".depot", ".depot.yaml"];

        for filename in filenames {
            let manifest_path = extracted_path.join(filename);
            if manifest_path.exists() {
                let content = fs::read_to_string(&manifest_path)?;
                let manifest: PackageManifest = serde_yaml::from_str(&content)
                    .map_err(|e| DepotError::Package(format!("Invalid package.yaml: {}", e)))?;
                return Ok(manifest);
            }
        }

        Err(DepotError::Package("No package.yaml found".to_string()))
    }

    /// Install files from extracted package
    fn install_from_extracted(
        &self,
        source_path: &Path,
        package_name: &str,
        _manifest: Option<&PackageManifest>,
    ) -> DepotResult<()> {
        let dest = self.lua_modules.join(package_name);
        fs::create_dir_all(&dest)?;

        // Default: Copy all Lua files and common directories
        self.install_default(source_path, &dest)
    }

    /// Default installation: copy Lua files and common directories
    fn install_default(&self, source_path: &Path, dest: &Path) -> DepotResult<()> {
        // Common patterns for Lua packages
        let lua_dirs = vec!["lib", "src", "lua"];

        // Try to copy common Lua directories
        for dir_name in &lua_dirs {
            let src_dir = source_path.join(dir_name);
            if src_dir.exists() && src_dir.is_dir() {
                let dest_dir = dest.join(dir_name);
                copy_dir_recursive(&src_dir, &dest_dir)?;
            }
        }

        // Copy any .lua files in the root
        for entry in WalkDir::new(source_path)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "lua") {
                let file_name = path.file_name().unwrap();
                fs::copy(path, dest.join(file_name))?;
            }
        }

        Ok(())
    }

    /// Check if a package is installed
    pub fn is_installed(&self, package_name: &str) -> bool {
        let package_dir = self.lua_modules.join(package_name);
        package_dir.exists() && package_dir.is_dir()
    }

    /// Remove an installed package
    pub fn remove_package(&self, package_name: &str) -> DepotResult<()> {
        let package_dir = self.lua_modules.join(package_name);

        if !package_dir.exists() {
            return Err(DepotError::Package(format!(
                "Package '{}' is not installed",
                package_name
            )));
        }

        // Remove package directory
        fs::remove_dir_all(&package_dir)?;

        // Remove metadata file if it exists
        let metadata_file = self.packages_dir.join(format!("{}.yaml", package_name));
        if metadata_file.exists() {
            fs::remove_file(&metadata_file)?;
        }

        Ok(())
    }

    /// Create a lockfile entry for an installed package
    pub fn create_lockfile_entry(
        &self,
        repository: &str,
        resolved: &ResolvedVersion,
        dependencies: &std::collections::HashMap<
            String,
            depot_core::package::manifest::DependencySpec,
        >,
    ) -> DepotResult<LockedPackage> {
        // Get tarball path to calculate checksum and size
        let parts: Vec<&str> = repository.split('/').collect();
        let _owner = parts[0];
        let _repo = parts[1];

        // We need the tarball path - this would have been cached from install_package
        // For now, we'll use the cache's source_path method
        let tarball_url = resolved.tarball_url.clone();
        let tarball_path = self.cache.source_path(&tarball_url);

        let checksum = self.cache.checksum(&tarball_path)?;
        let size = fs::metadata(&tarball_path)?.len();

        // Convert dependencies to simple map
        let dep_map: std::collections::HashMap<String, String> = dependencies
            .iter()
            .map(|(k, v)| {
                let version_str = v.version.clone().unwrap_or_else(|| "latest".to_string());
                (k.clone(), version_str)
            })
            .collect();

        Ok(LockedPackage {
            version: resolved.ref_value.clone(),
            repository: repository.to_string(),
            ref_type: format!("{:?}", resolved.ref_type),
            ref_value: resolved.ref_value.clone(),
            commit_sha: resolved.commit_sha.clone(),
            tarball_url: resolved.tarball_url.clone(),
            checksum,
            size,
            dependencies: dep_map,
            build: None,
            native_code: None,
        })
    }
}

/// Copy a directory recursively
fn copy_dir_recursive(src: &Path, dst: &Path) -> DepotResult<()> {
    fs::create_dir_all(dst)?;

    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let relative = path
            .strip_prefix(src)
            .map_err(|e| DepotError::Path(e.to_string()))?;
        let dest_path = dst.join(relative);

        if path.is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &dest_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::di::mocks::{MockCacheProvider, MockGitHubProvider};
    use crate::github::types::{RefType, ResolvedVersion};
    use tempfile::TempDir;

    #[test]
    fn test_installer_new() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let installer = PackageInstaller::new(temp.path(), cache, github, fallback).unwrap();
        assert_eq!(installer.project_root, temp.path());
    }

    #[test]
    fn test_installer_init() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let installer = PackageInstaller::new(temp.path(), cache, github, fallback).unwrap();
        installer.init().unwrap();

        assert!(installer.lua_modules.exists());
        assert!(installer.metadata_dir.exists());
        assert!(installer.packages_dir.exists());
    }

    #[tokio::test]
    async fn test_install_package_invalid_format() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let installer = PackageInstaller::new(temp.path(), cache, github, fallback).unwrap();
        installer.init().unwrap();

        let result = installer.install_package("invalid", None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid repository format"));
    }

    #[test]
    fn test_create_lockfile_entry() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        // Add a cached tarball
        let tarball_url = "https://api.github.com/repos/owner/repo/tarball/v1.0.0";
        let tarball_path = cache.source_path(tarball_url);
        cache.add_file(tarball_path.clone(), b"test tarball content".to_vec());

        // Ensure the file exists on disk for the test
        std::fs::create_dir_all(tarball_path.parent().unwrap()).unwrap();
        std::fs::write(&tarball_path, b"test tarball content").unwrap();

        let installer = PackageInstaller::new(temp.path(), cache, github, fallback).unwrap();

        let resolved = ResolvedVersion {
            ref_type: RefType::Release,
            ref_value: "v1.0.0".to_string(),
            commit_sha: "abc123".to_string(),
            tarball_url: tarball_url.to_string(),
        };

        let deps = std::collections::HashMap::new();
        let entry = installer
            .create_lockfile_entry("owner/repo", &resolved, &deps)
            .unwrap();

        assert_eq!(entry.repository, "owner/repo");
        assert_eq!(entry.version, "v1.0.0");
    }

    #[test]
    fn test_copy_dir_recursive() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("file1.txt"), "content1").unwrap();
        fs::create_dir_all(src.join("subdir")).unwrap();
        fs::write(src.join("subdir/file2.txt"), "content2").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(dst.exists());
        assert!(dst.join("file1.txt").exists());
        assert!(dst.join("subdir").exists());
        assert!(dst.join("subdir/file2.txt").exists());
    }

    #[test]
    fn test_read_package_manifest_not_found() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let installer = PackageInstaller::new(temp.path(), cache, github, fallback).unwrap();
        let result = installer.read_package_manifest(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_install_default() {
        let temp = TempDir::new().unwrap();
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let installer = PackageInstaller::new(temp.path(), cache, github, fallback).unwrap();

        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir_all(&src).unwrap();

        // Create some Lua files
        fs::write(src.join("init.lua"), "-- init").unwrap();
        fs::create_dir_all(src.join("lib")).unwrap();
        fs::write(src.join("lib/module.lua"), "-- module").unwrap();

        installer.install_default(&src, &dst).unwrap();

        assert!(dst.join("init.lua").exists());
        assert!(dst.join("lib").exists());
        assert!(dst.join("lib/module.lua").exists());
    }
}
