use lpm::core::{LpmError, LpmResult};
use lpm::workspace::Workspace;
use std::env;

/// List all packages in the workspace
pub async fn list() -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let workspace = Workspace::load(&current_dir)?;

    println!("Workspace: {}", workspace.config.name);
    println!("Root: {}", workspace.root.display());
    println!("\nPackages:");

    let mut packages: Vec<_> = workspace.packages.values().collect();
    packages.sort_by_key(|p| &p.name);

    for package in packages {
        println!("  {} ({})", package.name, package.path.display());
    }

    println!("\nTotal: {} packages", workspace.packages.len());

    Ok(())
}

/// Show detailed workspace information
pub async fn info() -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let workspace = Workspace::load(&current_dir)?;

    println!("Workspace: {}", workspace.config.name);
    println!("Root: {}", workspace.root.display());
    println!();

    // Package patterns
    println!("Package patterns:");
    for pattern in &workspace.config.packages {
        println!("  - {}", pattern);
    }
    println!();

    // Exclude patterns
    if !workspace.config.exclude.is_empty() {
        println!("Exclude patterns:");
        for pattern in &workspace.config.exclude {
            println!("  - {}", pattern);
        }
        println!();
    }

    // Default members
    if let Some(default_members) = &workspace.config.default_members {
        println!("Default members:");
        for member in default_members {
            println!("  - {}", member);
        }
        println!();
    }

    // Workspace dependencies
    if !workspace.config.dependencies.is_empty() {
        println!("Workspace dependencies:");
        for (name, version) in &workspace.config.dependencies {
            println!("  {} = {}", name, version);
        }
        println!();
    }

    // Workspace dev dependencies
    if !workspace.config.dev_dependencies.is_empty() {
        println!("Workspace dev dependencies:");
        for (name, version) in &workspace.config.dev_dependencies {
            println!("  {} = {}", name, version);
        }
        println!();
    }

    // Package metadata
    if let Some(metadata) = &workspace.config.package_metadata {
        println!("Workspace package metadata:");
        if let Some(version) = &metadata.version {
            println!("  version: {}", version);
        }
        if let Some(license) = &metadata.license {
            println!("  license: {}", license);
        }
        if let Some(authors) = &metadata.authors {
            println!("  authors: {}", authors.join(", "));
        }
        println!();
    }

    // Packages
    println!("Packages ({}):", workspace.packages.len());
    let mut packages: Vec<_> = workspace.packages.values().collect();
    packages.sort_by_key(|p| &p.name);

    for package in packages {
        println!("  {}", package.name);
        println!("    path: {}", package.path.display());
        println!("    version: {}", package.manifest.version);
        if !package.manifest.dependencies.is_empty() {
            println!("    dependencies: {}", package.manifest.dependencies.len());
        }
        if !package.manifest.dev_dependencies.is_empty() {
            println!(
                "    dev-dependencies: {}",
                package.manifest.dev_dependencies.len()
            );
        }
    }

    Ok(())
}

/// Show shared dependencies across workspace packages
pub async fn shared_deps() -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let workspace = Workspace::load(&current_dir)?;

    let shared = workspace.shared_dependencies();

    if shared.is_empty() {
        println!("No shared dependencies found across workspace packages.");
        return Ok(());
    }

    println!("Shared dependencies across workspace:");
    println!();

    let mut shared_vec: Vec<_> = shared.iter().collect();
    shared_vec.sort_by_key(|(name, _)| *name);

    for (dep_name, usages) in shared_vec {
        println!("{}:", dep_name);
        for (package_name, version) in usages {
            println!("  {} uses {}", package_name, version);
        }

        // Check for version conflicts
        let unique_versions: std::collections::HashSet<&str> =
            usages.iter().map(|(_, v)| v.as_str()).collect();
        if unique_versions.len() > 1 {
            println!("  ⚠️  Version conflict detected!");
        }
        println!();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Guard that restores the current directory when dropped
    struct DirGuard {
        original_dir: PathBuf,
    }

    impl DirGuard {
        fn new(new_dir: &std::path::Path) -> std::io::Result<Self> {
            let original_dir = env::current_dir()?;
            env::set_current_dir(new_dir)?;
            Ok(Self { original_dir })
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original_dir);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_list_empty_workspace() {
        let temp = TempDir::new().unwrap();

        // Create workspace.yaml
        fs::write(
            temp.path().join("workspace.yaml"),
            r#"
name: test-workspace
packages:
  - packages/*
"#,
        )
        .unwrap();

        let _guard = DirGuard::new(temp.path()).unwrap();

        let result = list().await;

        assert!(result.is_ok());
    }
}
