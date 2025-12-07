use crate::core::LpmResult;
use crate::workspace::Workspace;
use std::path::{Path, PathBuf};

/// Finds workspace root from a given directory
pub struct WorkspaceFinder;

impl WorkspaceFinder {
    /// Find workspace root by walking up the directory tree
    ///
    /// Looks for workspace.yaml or package.yaml with workspace configuration
    pub fn find_workspace_root(start_dir: &Path) -> LpmResult<Option<PathBuf>> {
        let mut current = start_dir.to_path_buf();

        loop {
            // Check for workspace.yaml
            if current.join("workspace.yaml").exists() {
                return Ok(Some(current));
            }

            // Check for package.yaml with workspace config
            let package_yaml = current.join("package.yaml");
            if package_yaml.exists() && Workspace::is_workspace(&current) {
                return Ok(Some(current));
            }

            // Move to parent directory
            match current.parent() {
                Some(parent) => current = parent.to_path_buf(),
                None => break, // Reached filesystem root
            }
        }

        Ok(None)
    }

    /// Find all package.yaml files in a workspace
    pub fn find_package_manifests(workspace_root: &Path) -> LpmResult<Vec<PathBuf>> {
        use walkdir::WalkDir;

        let mut manifests = Vec::new();

        for entry in WalkDir::new(workspace_root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "package.yaml" && entry.file_type().is_file() {
                manifests.push(entry.path().to_path_buf());
            }
        }

        Ok(manifests)
    }

    /// Check if a directory is within a workspace
    pub fn is_in_workspace(dir: &Path) -> bool {
        Self::find_workspace_root(dir).ok().flatten().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_workspace_root() {
        let temp = TempDir::new().unwrap();
        let workspace_yaml = temp.path().join("workspace.yaml");
        fs::write(&workspace_yaml, "name: test\npackages: []\n").unwrap();

        let subdir = temp.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        let root = WorkspaceFinder::find_workspace_root(&subdir).unwrap();
        assert_eq!(root, Some(temp.path().to_path_buf()));
    }

    #[test]
    fn test_find_workspace_root_with_workspace_yaml() {
        let temp = TempDir::new().unwrap();
        let workspace_dir = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();
        std::fs::write(workspace_dir.join("workspace.yaml"), "name: test").unwrap();

        let result = WorkspaceFinder::find_workspace_root(&workspace_dir);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_find_workspace_root_nonexistent() {
        let temp = TempDir::new().unwrap();
        let some_dir = temp.path().join("some-dir");
        std::fs::create_dir_all(&some_dir).unwrap();

        let result = WorkspaceFinder::find_workspace_root(&some_dir);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_find_package_manifests_empty() {
        let temp = TempDir::new().unwrap();
        let result = WorkspaceFinder::find_package_manifests(temp.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_find_package_manifests_with_packages() {
        let temp = TempDir::new().unwrap();
        let pkg1_dir = temp.path().join("pkg1");
        let pkg2_dir = temp.path().join("pkg2");
        fs::create_dir_all(&pkg1_dir).unwrap();
        fs::create_dir_all(&pkg2_dir).unwrap();

        fs::write(
            pkg1_dir.join("package.yaml"),
            "name: pkg1\nversion: 1.0.0\n",
        )
        .unwrap();
        fs::write(
            pkg2_dir.join("package.yaml"),
            "name: pkg2\nversion: 1.0.0\n",
        )
        .unwrap();

        let manifests = WorkspaceFinder::find_package_manifests(temp.path()).unwrap();
        assert_eq!(manifests.len(), 2);
    }

    #[test]
    fn test_find_package_manifests_nested() {
        let temp = TempDir::new().unwrap();
        let nested_dir = temp.path().join("nested").join("deep");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(
            nested_dir.join("package.yaml"),
            "name: nested\nversion: 1.0.0\n",
        )
        .unwrap();

        let manifests = WorkspaceFinder::find_package_manifests(temp.path()).unwrap();
        assert_eq!(manifests.len(), 1);
    }

    #[test]
    fn test_is_in_workspace_true() {
        let temp = TempDir::new().unwrap();
        let workspace_yaml = temp.path().join("workspace.yaml");
        fs::write(&workspace_yaml, "name: test\npackages: []\n").unwrap();

        let subdir = temp.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        assert!(WorkspaceFinder::is_in_workspace(&subdir));
    }

    #[test]
    fn test_is_in_workspace_false() {
        let temp = TempDir::new().unwrap();
        let some_dir = temp.path().join("some-dir");
        fs::create_dir_all(&some_dir).unwrap();

        assert!(!WorkspaceFinder::is_in_workspace(&some_dir));
    }

    #[test]
    fn test_find_workspace_root_with_package_yaml_workspace() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.yaml"),
            r#"
name: workspace
version: 1.0.0
workspace:
  packages:
    - packages/*
"#,
        )
        .unwrap();

        let subdir = temp.path().join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        let root = WorkspaceFinder::find_workspace_root(&subdir).unwrap();
        assert!(root.is_some());
    }
}
