use crate::core::error::{DepotError, DepotResult};
use std::path::{Path, PathBuf};

/// Get the Depot home directory
///
/// Platform-specific locations:
/// - Windows: %APPDATA%\depot
/// - Linux: ~/.config/depot
/// - macOS: ~/Library/Application Support/depot
pub fn depot_home() -> DepotResult<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| DepotError::Path("Could not determine config directory".to_string()))?;
    Ok(config_dir.join("depot"))
}

/// Get the cache directory
///
/// Platform-specific locations:
/// - Windows: %LOCALAPPDATA%\depot\cache
/// - Linux: ~/.cache/depot
/// - macOS: ~/Library/Caches/depot
pub fn cache_dir() -> DepotResult<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| DepotError::Path("Could not determine cache directory".to_string()))?;
    Ok(cache_dir.join("depot"))
}

/// Get the config file path
///
/// Platform-specific locations:
/// - Windows: %APPDATA%\depot\config.yaml
/// - Linux: ~/.config/depot/config.yaml
/// - macOS: ~/Library/Application Support/depot/config.yaml
pub fn config_file() -> DepotResult<PathBuf> {
    Ok(depot_home()?.join("config.yaml"))
}

/// Get the credentials file path (deprecated - use CredentialStore instead)
///
/// Platform-specific locations:
/// - Windows: %APPDATA%\depot\credentials
/// - Linux: ~/.config/depot/credentials
/// - macOS: ~/Library/Application Support/depot/credentials
///
/// Note: Depot uses OS keychain for credential storage. This path is kept
/// for compatibility but should not be used. If any credential files exist,
/// they should have 0600 permissions.
pub fn credentials_file() -> DepotResult<PathBuf> {
    Ok(depot_home()?.join("credentials"))
}

/// Get the Lua modules directory for the current project (./lua_modules)
pub fn lua_modules_dir(project_root: &Path) -> PathBuf {
    project_root.join("lua_modules")
}

/// Get the Depot metadata directory (./lua_modules/.depot)
pub fn depot_metadata_dir(project_root: &Path) -> PathBuf {
    lua_modules_dir(project_root).join(".depot")
}

/// Get the packages metadata directory (./lua_modules/.depot/packages)
pub fn packages_metadata_dir(project_root: &Path) -> PathBuf {
    depot_metadata_dir(project_root).join("packages")
}

/// Get the global installation directory
///
/// Platform-specific locations:
/// - Windows: %APPDATA%\depot\global
/// - Linux: ~/.config/depot/global
/// - macOS: ~/Library/Application Support/depot/global
pub fn global_dir() -> DepotResult<PathBuf> {
    Ok(depot_home()?.join("global"))
}

/// Get the global Lua modules directory
pub fn global_lua_modules_dir() -> DepotResult<PathBuf> {
    Ok(global_dir()?.join("lua_modules"))
}

/// Get the global bin directory (for executables)
///
/// This is the same as ~/.depot/bin/ (where Lua wrappers are)
pub fn global_bin_dir() -> DepotResult<PathBuf> {
    Ok(depot_home()?.join("bin"))
}

/// Get the global packages metadata directory
pub fn global_packages_metadata_dir() -> DepotResult<PathBuf> {
    Ok(global_dir()?.join(".depot").join("packages"))
}

/// Find the project root by looking for package.yaml or workspace.yaml
///
/// Checks for workspace.yaml first, then falls back to package.yaml.
/// Workspace detection is done by checking for workspace.yaml file or
/// package.yaml with workspace configuration.
pub fn find_project_root(start: &Path) -> DepotResult<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        // Check for workspace.yaml first
        let workspace_yaml = current.join("workspace.yaml");
        if workspace_yaml.exists() {
            return Ok(current);
        }

        // Check for package.yaml (which may contain workspace config)
        let package_yaml = current.join("package.yaml");
        if package_yaml.exists() {
            // Check if this package.yaml has workspace configuration
            // by looking for a "workspace" key (simple heuristic)
            if let Ok(content) = std::fs::read_to_string(&package_yaml) {
                if content.contains("workspace:") || content.contains("workspaces:") {
                    return Ok(current);
                }
            }
            // Regular package.yaml found
            return Ok(current);
        }

        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            return Err(DepotError::Path(
                "Could not find package.yaml or workspace.yaml in current directory or parents"
                    .to_string(),
            ));
        }
    }
}

/// Check if we're in a Depot project (package.yaml exists)
pub fn is_project_root(dir: &Path) -> bool {
    dir.join("package.yaml").exists()
}

/// Ensure a directory exists, creating it if necessary
pub fn ensure_dir(path: &Path) -> DepotResult<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Normalize a path for cross-platform compatibility
pub fn normalize_path(path: &Path) -> PathBuf {
    // Convert to string and back to handle path separators
    path.components().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root() {
        let temp = TempDir::new().unwrap();
        let project_dir = temp.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("package.yaml"), "name: test\n").unwrap();

        let found = find_project_root(&project_dir.join("subdir")).unwrap();
        assert_eq!(found, project_dir);
    }

    #[test]
    fn test_ensure_dir() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("test_dir");

        ensure_dir(&dir).unwrap();
        assert!(dir.exists());
        assert!(dir.is_dir());
    }
}
