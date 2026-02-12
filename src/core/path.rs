use crate::core::{DepotError, DepotResult};
use std::path::{Path, PathBuf};

// Re-export all path functions from lpm-core
pub use depot_core::core::path::*;

/// Find the project root by looking for package.yaml or workspace.yaml
/// 
/// This is an enhanced version that includes workspace support.
/// It checks for workspace.yaml first, then falls back to package.yaml.
pub fn find_project_root(start: &Path) -> DepotResult<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        // Check for workspace first (workspace.yaml or package.yaml with workspace config)
        if crate::workspace::Workspace::is_workspace(&current) {
            return Ok(current);
        }

        // Check for regular package.yaml
        let package_yaml = current.join("package.yaml");
        if package_yaml.exists() {
            return Ok(current);
        }

        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            return Err(DepotError::Path(
                "Could not find package.yaml or workspace.yaml in current directory or parents".to_string(),
            ));
        }
    }
}


