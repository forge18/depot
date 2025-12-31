use crate::core::{LpmError, LpmResult};
use std::fs;
use std::path::{Path, PathBuf};

pub struct VersionSwitcher {
    versions_dir: PathBuf,
    current_file: PathBuf,
}

impl VersionSwitcher {
    pub fn new(lpm_home: &Path) -> Self {
        Self {
            versions_dir: lpm_home.join("versions"),
            current_file: lpm_home.join("current"),
        }
    }

    /// Switch to a specific version globally
    pub fn switch(&self, version: &str) -> LpmResult<()> {
        let target = self.versions_dir.join(version);

        if !target.exists() {
            let installed = self.list_installed()?;
            return Err(LpmError::Package(format!(
                "Lua {} is not installed.\n\
                 Installed versions: {}\n\
                 Run: lpm lua install {}",
                version,
                installed.join(", "),
                version
            )));
        }

        // Verify installation is complete
        let lua_bin = target.join("bin").join("lua");
        if !lua_bin.exists() {
            return Err(LpmError::Package(format!(
                "Lua {} installation is incomplete (missing lua binary)",
                version
            )));
        }

        // Write version to current file (simple text file, not symlink)
        fs::write(&self.current_file, format!("{}\n", version))?;

        println!("✓ Now using Lua {}", version);

        // Verify switch worked
        self.verify_current_version(version)?;

        Ok(())
    }

    /// Set version for current project (creates .lua-version file)
    pub fn set_local(&self, version: &str, project_dir: &Path) -> LpmResult<()> {
        // Verify version is installed
        let target = self.versions_dir.join(version);
        if !target.exists() {
            return Err(LpmError::Package(format!(
                "Lua {} is not installed. Run: lpm lua install {}",
                version, version
            )));
        }

        // Write .lua-version file
        let version_file = project_dir.join(".lua-version");
        fs::write(&version_file, format!("{}\n", version))?;

        println!("✓ Set project Lua version to {}", version);
        println!("  (stored in {})", version_file.display());

        Ok(())
    }

    /// Get currently active version
    pub fn current(&self) -> LpmResult<String> {
        if !self.current_file.exists() {
            return Err(LpmError::Package(
                "No Lua version is currently selected.\n\
                 Run: lpm lua use <version>"
                    .to_string(),
            ));
        }

        let version = fs::read_to_string(&self.current_file)?.trim().to_string();

        if version.is_empty() {
            return Err(LpmError::Package(
                "Current version file is empty.\n\
                 Run: lpm lua use <version>"
                    .to_string(),
            ));
        }

        // Verify version is still installed
        if !self.versions_dir.join(&version).exists() {
            return Err(LpmError::Package(format!(
                "Current version {} is no longer installed.\n\
                 Run: lpm lua use <version>",
                version
            )));
        }

        Ok(version)
    }

    /// List all installed versions
    pub fn list_installed(&self) -> LpmResult<Vec<String>> {
        if !self.versions_dir.exists() {
            return Ok(vec![]);
        }

        let mut versions = Vec::new();
        let entries = fs::read_dir(&self.versions_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Verify it's a valid installation
                let lua_bin = path.join("bin").join("lua");
                if lua_bin.exists() {
                    if let Some(version) = path.file_name().and_then(|n| n.to_str()) {
                        versions.push(version.to_string());
                    }
                }
            }
        }

        versions.sort();
        Ok(versions)
    }

    /// Verify that the current version file points to the expected version
    fn verify_current_version(&self, expected: &str) -> LpmResult<()> {
        let actual = self.current()?;
        if actual != expected {
            return Err(LpmError::Package(format!(
                "Version switch verification failed: expected {}, got {}",
                expected, actual
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_version_switcher_new() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());
        assert_eq!(switcher.versions_dir, temp.path().join("versions"));
        assert_eq!(switcher.current_file, temp.path().join("current"));
    }

    #[test]
    fn test_version_switcher_list_installed_empty() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());
        let versions = switcher.list_installed().unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn test_version_switcher_list_installed_with_versions() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());

        // Create a mock installation
        let version_dir = temp.path().join("versions").join("5.4.8");
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(version_dir.join("bin").join("lua"), "mock binary").unwrap();

        let versions = switcher.list_installed().unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0], "5.4.8");
    }

    #[test]
    fn test_version_switcher_current_not_set() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());
        let result = switcher.current();
        assert!(result.is_err());
    }

    #[test]
    fn test_version_switcher_current_set() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());

        // Create version directory and current file
        let version_dir = temp.path().join("versions").join("5.4.8");
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(version_dir.join("bin").join("lua"), "mock binary").unwrap();
        fs::write(temp.path().join("current"), "5.4.8\n").unwrap();

        let current = switcher.current().unwrap();
        assert_eq!(current, "5.4.8");
    }

    #[test]
    fn test_version_switcher_switch_version_not_installed() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());
        let result = switcher.switch("5.4.8");
        assert!(result.is_err());
    }

    #[test]
    fn test_version_switcher_set_local() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());
        let project_dir = temp.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();

        // Create version directory
        let version_dir = temp.path().join("versions").join("5.4.8");
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(version_dir.join("bin").join("lua"), "mock binary").unwrap();

        switcher.set_local("5.4.8", &project_dir).unwrap();
        assert!(project_dir.join(".lua-version").exists());
        let content = fs::read_to_string(project_dir.join(".lua-version")).unwrap();
        assert_eq!(content.trim(), "5.4.8");
    }

    #[test]
    fn test_version_switcher_switch_success() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());

        // Create version directory with proper structure
        let version_dir = temp.path().join("versions").join("5.4.8");
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(version_dir.join("bin").join("lua"), "mock binary").unwrap();

        let result = switcher.switch("5.4.8");
        assert!(result.is_ok());

        // Verify current file was created
        assert!(temp.path().join("current").exists());
        let content = fs::read_to_string(temp.path().join("current")).unwrap();
        assert_eq!(content.trim(), "5.4.8");
    }

    #[test]
    fn test_version_switcher_list_multiple_versions() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());

        // Create multiple version directories
        for version in &["5.1.5", "5.3.6", "5.4.8"] {
            let version_dir = temp.path().join("versions").join(version);
            fs::create_dir_all(version_dir.join("bin")).unwrap();
            fs::write(version_dir.join("bin").join("lua"), "mock binary").unwrap();
        }

        let versions = switcher.list_installed().unwrap();
        assert_eq!(versions.len(), 3);
        assert!(versions.contains(&"5.1.5".to_string()));
        assert!(versions.contains(&"5.3.6".to_string()));
        assert!(versions.contains(&"5.4.8".to_string()));
    }

    #[test]
    fn test_version_switcher_set_local_version_not_installed() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());
        let project_dir = temp.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();

        let result = switcher.set_local("5.4.8", &project_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_version_switcher_check_installed_via_list() {
        let temp = TempDir::new().unwrap();
        let switcher = VersionSwitcher::new(temp.path());

        // Create version directory
        let version_dir = temp.path().join("versions").join("5.4.8");
        fs::create_dir_all(version_dir.join("bin")).unwrap();
        fs::write(version_dir.join("bin").join("lua"), "mock binary").unwrap();

        let installed = switcher.list_installed().unwrap();
        assert!(installed.contains(&"5.4.8".to_string()));
        assert!(!installed.contains(&"5.3.6".to_string()));
    }
}
