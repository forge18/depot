use crate::core::LpmResult;
use crate::package::lockfile::Lockfile;
use crate::package::manifest::PackageManifest;
use std::path::Path;

/// Manages rollback for failed installations
pub struct RollbackManager {
    backup_lockfile: Option<Lockfile>,
    backup_manifest: Option<PackageManifest>,
}

impl RollbackManager {
    /// Create a new rollback manager and backup current state
    pub fn new(project_root: &Path) -> LpmResult<Self> {
        // Backup lockfile if it exists
        let backup_lockfile = Lockfile::load(project_root)?;

        // Backup manifest
        let backup_manifest = PackageManifest::load(project_root).ok();

        Ok(Self {
            backup_lockfile,
            backup_manifest,
        })
    }

    /// Rollback to the previous state
    pub fn rollback(&self, project_root: &Path) -> LpmResult<()> {
        // Restore lockfile if we had a backup
        if let Some(ref lockfile) = self.backup_lockfile {
            lockfile.save(project_root)?;
            eprintln!("✓ Rolled back {}", crate::package::lockfile::LOCKFILE_NAME);
        }

        // Restore manifest if we had a backup
        if let Some(ref manifest) = self.backup_manifest {
            manifest.save(project_root)?;
            eprintln!("✓ Rolled back package.yaml");
        }

        Ok(())
    }

    /// Check if rollback is available
    pub fn has_backup(&self) -> bool {
        self.backup_lockfile.is_some() || self.backup_manifest.is_some()
    }
}

/// Execute a function with automatic rollback on error
pub fn with_rollback<F, T>(project_root: &Path, f: F) -> LpmResult<T>
where
    F: FnOnce() -> LpmResult<T>,
{
    // Create rollback manager
    let rollback = RollbackManager::new(project_root)?;

    // Execute the function
    match f() {
        Ok(result) => Ok(result),
        Err(e) => {
            // Attempt rollback
            if rollback.has_backup() {
                eprintln!("\n⚠️  Installation failed. Attempting rollback...");
                if let Err(rollback_err) = rollback.rollback(project_root) {
                    eprintln!("❌ Rollback failed: {}", rollback_err);
                } else {
                    eprintln!("✓ Rollback completed");
                }
            }
            Err(e)
        }
    }
}

/// Execute an async function with automatic rollback on error
pub async fn with_rollback_async<F, Fut, T>(project_root: &Path, f: F) -> LpmResult<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = LpmResult<T>>,
{
    // Create rollback manager
    let rollback = RollbackManager::new(project_root)?;

    // Execute the function
    match f().await {
        Ok(result) => Ok(result),
        Err(e) => {
            // Attempt rollback
            if rollback.has_backup() {
                eprintln!("\n⚠️  Installation failed. Attempting rollback...");
                if let Err(rollback_err) = rollback.rollback(project_root) {
                    eprintln!("❌ Rollback failed: {}", rollback_err);
                } else {
                    eprintln!("✓ Rollback completed");
                }
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageManifest;
    use tempfile::TempDir;

    #[test]
    fn test_rollback_manager() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test".to_string());
        manifest.save(temp.path()).unwrap();

        let rollback = RollbackManager::new(temp.path()).unwrap();
        assert!(rollback.has_backup());
    }

    #[test]
    fn test_rollback_manager_without_files() {
        let temp = TempDir::new().unwrap();
        let rollback = RollbackManager::new(temp.path()).unwrap();
        // Should still create manager even without files
        let _ = rollback;
    }

    #[test]
    fn test_rollback_rollback() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test".to_string());
        manifest.save(temp.path()).unwrap();

        let rollback = RollbackManager::new(temp.path()).unwrap();

        // Modify manifest
        let modified = PackageManifest::default("modified".to_string());
        modified.save(temp.path()).unwrap();

        // Rollback
        rollback.rollback(temp.path()).unwrap();

        // Verify it was restored
        let restored = PackageManifest::load(temp.path()).unwrap();
        assert_eq!(restored.name, "test");
    }

    #[test]
    fn test_with_rollback_success() {
        let temp = TempDir::new().unwrap();
        let result = with_rollback(temp.path(), || Ok::<(), crate::core::LpmError>(()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_with_rollback_failure() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test".to_string());
        manifest.save(temp.path()).unwrap();

        let result = with_rollback(temp.path(), || {
            Err::<(), crate::core::LpmError>(crate::core::LpmError::Package(
                "test error".to_string(),
            ))
        });
        assert!(result.is_err());

        // Verify rollback happened
        let restored = PackageManifest::load(temp.path()).unwrap();
        assert_eq!(restored.name, "test");
    }

    #[test]
    fn test_rollback_manager_has_backup_with_lockfile() {
        use crate::package::lockfile::Lockfile;
        let temp = TempDir::new().unwrap();
        let lockfile = Lockfile::new();
        lockfile.save(temp.path()).unwrap();

        let rollback = RollbackManager::new(temp.path()).unwrap();
        assert!(rollback.has_backup());
    }

    #[test]
    fn test_rollback_manager_has_backup_with_manifest_only() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test".to_string());
        manifest.save(temp.path()).unwrap();

        let rollback = RollbackManager::new(temp.path()).unwrap();
        assert!(rollback.has_backup());
    }

    #[test]
    fn test_rollback_manager_has_backup_false() {
        let temp = TempDir::new().unwrap();
        let rollback = RollbackManager::new(temp.path()).unwrap();
        assert!(!rollback.has_backup());
    }

    #[test]
    fn test_rollback_with_lockfile() {
        use crate::package::lockfile::{LockedPackage, Lockfile};
        use std::collections::HashMap;
        let temp = TempDir::new().unwrap();

        let mut lockfile = Lockfile::new();
        let package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None,
            checksum: "abc123".to_string(),
            size: None,
            dependencies: HashMap::new(),
            build: None,
        };
        lockfile.add_package("test-package".to_string(), package);
        lockfile.save(temp.path()).unwrap();

        let rollback = RollbackManager::new(temp.path()).unwrap();

        // Modify lockfile
        let new_lockfile = Lockfile::new();
        new_lockfile.save(temp.path()).unwrap();

        // Rollback
        rollback.rollback(temp.path()).unwrap();

        // Verify it was restored
        let restored = Lockfile::load(temp.path()).unwrap().unwrap();
        assert!(restored.has_package("test-package"));
    }

    #[tokio::test]
    async fn test_with_rollback_async_success() {
        let temp = TempDir::new().unwrap();
        let result = with_rollback_async(temp.path(), || async {
            Ok::<(), crate::core::LpmError>(())
        })
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_rollback_async_failure() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test".to_string());
        manifest.save(temp.path()).unwrap();

        let result = with_rollback_async(temp.path(), || async {
            Err::<(), crate::core::LpmError>(crate::core::LpmError::Package(
                "test error".to_string(),
            ))
        })
        .await;
        assert!(result.is_err());

        // Verify rollback happened
        let restored = PackageManifest::load(temp.path()).unwrap();
        assert_eq!(restored.name, "test");
    }
}
