use crate::core::{LpmError, LpmResult};
use crate::package::manifest::PackageManifest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Represents a workspace (monorepo) with multiple packages
pub struct Workspace {
    /// Root directory of the workspace
    pub root: PathBuf,
    /// Workspace configuration (from root package.yaml or workspace.yaml)
    pub config: WorkspaceConfig,
    /// All packages in the workspace, keyed by package name
    pub packages: HashMap<String, WorkspacePackage>,
}

/// Workspace configuration
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Workspace name
    pub name: String,
    /// Package directories (relative to workspace root)
    pub packages: Vec<String>,
}

/// A package within a workspace
#[derive(Debug, Clone)]
pub struct WorkspacePackage {
    /// Package name
    pub name: String,
    /// Path to package directory (relative to workspace root)
    pub path: PathBuf,
    /// Package manifest
    pub manifest: PackageManifest,
}

impl Workspace {
    /// Load a workspace from a directory
    pub fn load(workspace_root: &Path) -> LpmResult<Self> {
        // Check for workspace.yaml or package.yaml with workspace config
        let config = Self::load_config(workspace_root)?;

        // Find all packages in the workspace
        let packages = Self::find_packages(workspace_root, &config)?;

        Ok(Self {
            root: workspace_root.to_path_buf(),
            config,
            packages,
        })
    }

    /// Load workspace configuration
    fn load_config(workspace_root: &Path) -> LpmResult<WorkspaceConfig> {
        // Try workspace.yaml first
        let workspace_yaml = workspace_root.join("workspace.yaml");
        if workspace_yaml.exists() {
            return Self::load_workspace_yaml(&workspace_yaml);
        }

        // Try package.yaml with workspace field
        let package_yaml = workspace_root.join("package.yaml");
        if package_yaml.exists() {
            if let Ok(config) = Self::load_from_package_yaml(&package_yaml) {
                return Ok(config);
            }
        }

        // Default: auto-detect packages in common locations
        Ok(WorkspaceConfig {
            name: workspace_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string(),
            packages: vec!["packages/*".to_string(), "apps/*".to_string()],
        })
    }

    /// Load workspace.yaml
    fn load_workspace_yaml(path: &Path) -> LpmResult<WorkspaceConfig> {
        use serde::Deserialize;
        use std::fs;

        #[derive(Deserialize)]
        struct WorkspaceYaml {
            name: String,
            packages: Vec<String>,
        }

        let content = fs::read_to_string(path)?;
        let workspace: WorkspaceYaml = serde_yaml::from_str(&content)
            .map_err(|e| LpmError::Package(format!("Failed to parse workspace.yaml: {}", e)))?;

        Ok(WorkspaceConfig {
            name: workspace.name,
            packages: workspace.packages,
        })
    }

    /// Load workspace config from package.yaml
    fn load_from_package_yaml(path: &Path) -> LpmResult<WorkspaceConfig> {
        use serde::Deserialize;
        use std::fs;

        #[derive(Deserialize)]
        struct PackageYamlWithWorkspace {
            name: String,
            workspace: Option<WorkspaceYamlSection>,
        }

        #[derive(Deserialize)]
        struct WorkspaceYamlSection {
            packages: Vec<String>,
        }

        let content = fs::read_to_string(path)?;
        let package: PackageYamlWithWorkspace = serde_yaml::from_str(&content)
            .map_err(|e| LpmError::Package(format!("Failed to parse package.yaml: {}", e)))?;

        if let Some(workspace) = package.workspace {
            Ok(WorkspaceConfig {
                name: package.name,
                packages: workspace.packages,
            })
        } else {
            Err(LpmError::Package(
                "No workspace section in package.yaml".to_string(),
            ))
        }
    }

    /// Find all packages in the workspace
    fn find_packages(
        workspace_root: &Path,
        config: &WorkspaceConfig,
    ) -> LpmResult<HashMap<String, WorkspacePackage>> {
        use walkdir::WalkDir;

        let mut packages = HashMap::new();

        // For each pattern, find matching directories
        for pattern in &config.packages {
            // Handle glob patterns like "packages/*" or "apps/*"
            if pattern.contains('*') {
                // Extract base path before wildcard
                let base_path = if let Some(star_pos) = pattern.find('*') {
                    pattern[..star_pos].trim_end_matches('/')
                } else {
                    pattern
                };

                let search_dir = workspace_root.join(base_path);
                if search_dir.exists() {
                    // Walk directory to find package.yaml files
                    for entry in WalkDir::new(&search_dir)
                        .max_depth(3) // Limit depth to avoid deep recursion
                        .into_iter()
                        .filter_map(|e| e.ok())
                    {
                        if entry.file_name() == "package.yaml" && entry.file_type().is_file() {
                            if let Some(package_dir) = entry.path().parent() {
                                match PackageManifest::load(package_dir) {
                                    Ok(manifest) => {
                                        let package = WorkspacePackage {
                                            name: manifest.name.clone(),
                                            path: package_dir
                                                .strip_prefix(workspace_root)
                                                .unwrap_or(package_dir)
                                                .to_path_buf(),
                                            manifest,
                                        };
                                        packages.insert(package.name.clone(), package);
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Warning: Failed to load package at {}: {}",
                                            package_dir.display(),
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // Direct path (no wildcard)
                let package_dir = workspace_root.join(pattern);
                if package_dir.exists() {
                    match PackageManifest::load(&package_dir) {
                        Ok(manifest) => {
                            let package = WorkspacePackage {
                                name: manifest.name.clone(),
                                path: package_dir
                                    .strip_prefix(workspace_root)
                                    .unwrap_or(&package_dir)
                                    .to_path_buf(),
                                manifest,
                            };
                            packages.insert(package.name.clone(), package);
                        }
                        Err(e) => {
                            eprintln!(
                                "Warning: Failed to load package at {}: {}",
                                package_dir.display(),
                                e
                            );
                        }
                    }
                }
            }
        }

        Ok(packages)
    }

    /// Get a package by name
    pub fn get_package(&self, name: &str) -> Option<&WorkspacePackage> {
        self.packages.get(name)
    }

    /// Get all package names
    pub fn package_names(&self) -> Vec<String> {
        self.packages.keys().cloned().collect()
    }

    /// Get shared dependencies across all workspace packages
    ///
    /// Returns a map of package name to version constraint, where the same
    /// package appears in multiple workspace packages with potentially different constraints.
    pub fn shared_dependencies(&self) -> HashMap<String, Vec<(String, String)>> {
        let mut shared: HashMap<String, Vec<(String, String)>> = HashMap::new();

        for (package_name, workspace_pkg) in &self.packages {
            // Check regular dependencies
            for (dep_name, dep_version) in &workspace_pkg.manifest.dependencies {
                shared
                    .entry(dep_name.clone())
                    .or_default()
                    .push((package_name.clone(), dep_version.clone()));
            }

            // Check dev dependencies
            for (dep_name, dep_version) in &workspace_pkg.manifest.dev_dependencies {
                shared
                    .entry(dep_name.clone())
                    .or_default()
                    .push((package_name.clone(), format!("{} (dev)", dep_version)));
            }
        }

        // Filter to only dependencies used by multiple packages
        shared.retain(|_, usages| usages.len() > 1);
        shared
    }

    /// Check if a workspace is detected at the given path
    pub fn is_workspace(path: &Path) -> bool {
        path.join("workspace.yaml").exists()
            || (path.join("package.yaml").exists()
                && Self::load_from_package_yaml(&path.join("package.yaml")).is_ok())
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            name: "workspace".to_string(),
            packages: vec!["packages/*".to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_config_default() {
        let config = WorkspaceConfig::default();
        assert_eq!(config.name, "workspace");
        assert!(!config.packages.is_empty());
    }

    #[test]
    fn test_workspace_yaml_loading() {
        let temp = TempDir::new().unwrap();
        let workspace_yaml = temp.path().join("workspace.yaml");
        fs::write(
            &workspace_yaml,
            r#"
name: test-workspace
packages:
  - packages/*
  - apps/*
"#,
        )
        .unwrap();

        let config = Workspace::load_config(temp.path()).unwrap();
        assert_eq!(config.name, "test-workspace");
        assert_eq!(config.packages.len(), 2);
    }

    #[test]
    fn test_workspace_config_packages() {
        let mut config = WorkspaceConfig::default();
        let initial_len = config.packages.len();
        config.packages.push("test/*".to_string());
        assert_eq!(config.packages.len(), initial_len + 1);
    }

    #[test]
    fn test_workspace_load_config_missing_file() {
        let temp = TempDir::new().unwrap();
        // load_config returns default config when no files exist
        let result = Workspace::load_config(temp.path());
        assert!(result.is_ok()); // Returns default config, not an error
        let config = result.unwrap();
        assert_eq!(
            config.name,
            temp.path().file_name().unwrap().to_string_lossy()
        );
    }

    #[test]
    fn test_workspace_load_config_invalid_yaml() {
        let temp = TempDir::new().unwrap();
        let workspace_yaml = temp.path().join("workspace.yaml");
        fs::write(&workspace_yaml, "invalid: yaml: content: [").unwrap();

        let result = Workspace::load_config(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_workspace_config_name() {
        let config = WorkspaceConfig::default();
        assert_eq!(config.name, "workspace");
    }

    #[test]
    fn test_workspace_config_default_packages() {
        let config = WorkspaceConfig::default();
        // Default should have some packages
        assert!(!config.packages.is_empty());
    }

    #[test]
    fn test_workspace_find_packages_with_glob() {
        let temp = TempDir::new().unwrap();

        // Create packages directory structure
        let packages_dir = temp.path().join("packages");
        fs::create_dir_all(packages_dir.join("pkg1")).unwrap();
        fs::create_dir_all(packages_dir.join("pkg2")).unwrap();

        // Create package.yaml files
        fs::write(
            packages_dir.join("pkg1").join("package.yaml"),
            r#"
name: pkg1
version: 1.0.0
"#,
        )
        .unwrap();
        fs::write(
            packages_dir.join("pkg2").join("package.yaml"),
            r#"
name: pkg2
version: 1.0.0
"#,
        )
        .unwrap();

        let config = WorkspaceConfig {
            name: "test-workspace".to_string(),
            packages: vec!["packages/*".to_string()],
        };

        let packages = Workspace::find_packages(temp.path(), &config).unwrap();
        assert!(packages.len() >= 2);
        assert!(packages.contains_key("pkg1"));
        assert!(packages.contains_key("pkg2"));
    }

    #[test]
    fn test_workspace_find_packages_with_direct_path() {
        let temp = TempDir::new().unwrap();

        let package_dir = temp.path().join("my-package");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("package.yaml"),
            r#"
name: my-package
version: 1.0.0
"#,
        )
        .unwrap();

        let config = WorkspaceConfig {
            name: "test-workspace".to_string(),
            packages: vec!["my-package".to_string()],
        };

        let packages = Workspace::find_packages(temp.path(), &config).unwrap();
        assert_eq!(packages.len(), 1);
        assert!(packages.contains_key("my-package"));
    }

    #[test]
    fn test_workspace_shared_dependencies() {
        let temp = TempDir::new().unwrap();

        let pkg1_dir = temp.path().join("packages").join("pkg1");
        let pkg2_dir = temp.path().join("packages").join("pkg2");
        fs::create_dir_all(&pkg1_dir).unwrap();
        fs::create_dir_all(&pkg2_dir).unwrap();

        // Both packages depend on "shared-dep"
        fs::write(
            pkg1_dir.join("package.yaml"),
            r#"
name: pkg1
version: 1.0.0
dependencies:
  shared-dep: ">=1.0.0"
"#,
        )
        .unwrap();
        fs::write(
            pkg2_dir.join("package.yaml"),
            r#"
name: pkg2
version: 1.0.0
dependencies:
  shared-dep: ">=2.0.0"
"#,
        )
        .unwrap();

        let config = WorkspaceConfig {
            name: "test-workspace".to_string(),
            packages: vec!["packages/*".to_string()],
        };

        let workspace = Workspace {
            root: temp.path().to_path_buf(),
            config: config.clone(),
            packages: Workspace::find_packages(temp.path(), &config).unwrap(),
        };

        let shared = workspace.shared_dependencies();
        assert!(shared.contains_key("shared-dep"));
        assert_eq!(shared.get("shared-dep").unwrap().len(), 2);
    }

    #[test]
    fn test_workspace_shared_dependencies_with_dev() {
        let temp = TempDir::new().unwrap();

        let pkg1_dir = temp.path().join("packages").join("pkg1");
        fs::create_dir_all(&pkg1_dir).unwrap();

        fs::write(
            pkg1_dir.join("package.yaml"),
            r#"
name: pkg1
version: 1.0.0
dependencies:
  shared-dep: ">=1.0.0"
dev_dependencies:
  dev-dep: ">=1.0.0"
"#,
        )
        .unwrap();

        let config = WorkspaceConfig {
            name: "test-workspace".to_string(),
            packages: vec!["packages/*".to_string()],
        };

        let workspace = Workspace {
            root: temp.path().to_path_buf(),
            config: config.clone(),
            packages: Workspace::find_packages(temp.path(), &config).unwrap(),
        };

        let shared = workspace.shared_dependencies();
        // Dev dependencies used by only one package shouldn't be in shared
        assert!(!shared.contains_key("dev-dep"));
    }

    #[test]
    fn test_workspace_get_package() {
        let temp = TempDir::new().unwrap();

        let pkg_dir = temp.path().join("packages").join("pkg1");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("package.yaml"),
            r#"
name: pkg1
version: 1.0.0
"#,
        )
        .unwrap();

        let config = WorkspaceConfig {
            name: "test-workspace".to_string(),
            packages: vec!["packages/*".to_string()],
        };

        let workspace = Workspace {
            root: temp.path().to_path_buf(),
            config: config.clone(),
            packages: Workspace::find_packages(temp.path(), &config).unwrap(),
        };

        let pkg = workspace.get_package("pkg1");
        assert!(pkg.is_some());
        assert_eq!(pkg.unwrap().name, "pkg1");

        let missing = workspace.get_package("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_workspace_package_names() {
        let temp = TempDir::new().unwrap();

        let pkg1_dir = temp.path().join("packages").join("pkg1");
        let pkg2_dir = temp.path().join("packages").join("pkg2");
        fs::create_dir_all(&pkg1_dir).unwrap();
        fs::create_dir_all(&pkg2_dir).unwrap();

        fs::write(
            pkg1_dir.join("package.yaml"),
            r#"
name: pkg1
version: 1.0.0
"#,
        )
        .unwrap();
        fs::write(
            pkg2_dir.join("package.yaml"),
            r#"
name: pkg2
version: 1.0.0
"#,
        )
        .unwrap();

        let config = WorkspaceConfig {
            name: "test-workspace".to_string(),
            packages: vec!["packages/*".to_string()],
        };

        let workspace = Workspace {
            root: temp.path().to_path_buf(),
            config: config.clone(),
            packages: Workspace::find_packages(temp.path(), &config).unwrap(),
        };

        let names = workspace.package_names();
        assert!(names.len() >= 2);
        assert!(names.contains(&"pkg1".to_string()));
        assert!(names.contains(&"pkg2".to_string()));
    }

    #[test]
    fn test_workspace_is_workspace_with_workspace_yaml() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("workspace.yaml"),
            r#"
name: test-workspace
packages:
  - packages/*
"#,
        )
        .unwrap();

        assert!(Workspace::is_workspace(temp.path()));
    }

    #[test]
    fn test_workspace_is_workspace_with_package_yaml() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.yaml"),
            r#"
name: test-workspace
workspace:
  packages:
    - packages/*
"#,
        )
        .unwrap();

        assert!(Workspace::is_workspace(temp.path()));
    }

    #[test]
    fn test_workspace_is_workspace_false() {
        let temp = TempDir::new().unwrap();
        // No workspace.yaml or package.yaml with workspace section
        assert!(!Workspace::is_workspace(temp.path()));
    }

    #[test]
    fn test_workspace_load_from_package_yaml() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.yaml"),
            r#"
name: test-workspace
workspace:
  packages:
    - packages/*
    - apps/*
"#,
        )
        .unwrap();

        let config = Workspace::load_config(temp.path()).unwrap();
        assert_eq!(config.name, "test-workspace");
        assert_eq!(config.packages.len(), 2);
    }

    #[test]
    fn test_workspace_load_from_package_yaml_no_workspace_section() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.yaml"),
            r#"
name: test-package
version: 1.0.0
"#,
        )
        .unwrap();

        // Should return default config when no workspace section
        let config = Workspace::load_config(temp.path()).unwrap();
        // Should have default packages
        assert!(!config.packages.is_empty());
    }
}
