use depot::core::{DepotError, DepotResult};
use depot::workspace::Workspace;
use std::env;
use std::path::Path;

/// List all packages in the workspace
pub async fn list() -> DepotResult<()> {
    list_from_dir(None).await
}

/// List all packages in the workspace from a specific directory
/// Used internally for testing
async fn list_from_dir(dir: Option<&Path>) -> DepotResult<()> {
    let current_dir = match dir {
        Some(path) => path.to_path_buf(),
        None => env::current_dir()
            .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?,
    };

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
pub async fn info() -> DepotResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;

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
pub async fn shared_deps() -> DepotResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;

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
    use tempfile::TempDir;

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

        // Create packages directory (even though it's empty)
        fs::create_dir_all(temp.path().join("packages")).unwrap();

        let result = list_from_dir(Some(temp.path())).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_list_workspace_with_packages() {
        let temp = TempDir::new().unwrap();

        // Create workspace.yaml
        fs::write(
            temp.path().join("workspace.yaml"),
            r#"
name: my-workspace
packages:
  - packages/*
"#,
        )
        .unwrap();

        // Create a package
        let pkg_dir = temp.path().join("packages").join("pkg1");
        fs::create_dir_all(&pkg_dir).unwrap();
        fs::write(
            pkg_dir.join("package.yaml"),
            r#"
name: pkg1
version: 1.0.0
dependencies:
  lua: "5.1"
"#,
        )
        .unwrap();

        let result = list_from_dir(Some(temp.path())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_info_empty_workspace() {
        let temp = TempDir::new().unwrap();

        // Save current dir and change to temp
        let original_dir = env::current_dir().ok();
        env::set_current_dir(temp.path()).unwrap();

        // Create workspace.yaml
        fs::write(
            temp.path().join("workspace.yaml"),
            r#"
name: info-test
packages:
  - packages/*
"#,
        )
        .unwrap();

        fs::create_dir_all(temp.path().join("packages")).unwrap();

        let result = info().await;

        // Restore original dir
        if let Some(dir) = original_dir {
            let _ = env::set_current_dir(dir);
        }

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_info_workspace_with_metadata() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().ok();
        env::set_current_dir(temp.path()).unwrap();

        // Create workspace with metadata
        fs::write(
            temp.path().join("workspace.yaml"),
            r#"
name: meta-workspace
packages:
  - packages/*
exclude:
  - packages/ignore/*
dependencies:
  lua: "5.1"
  inspect: "3.0.0"
dev_dependencies:
  luaunit: "3.4"
package_metadata:
  version: "1.0.0"
  license: "MIT"
  authors:
    - "Test Author"
"#,
        )
        .unwrap();

        fs::create_dir_all(temp.path().join("packages")).unwrap();

        let result = info().await;

        if let Some(dir) = original_dir {
            let _ = env::set_current_dir(dir);
        }

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_shared_deps_no_packages() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().ok();
        env::set_current_dir(temp.path()).unwrap();

        fs::write(
            temp.path().join("workspace.yaml"),
            r#"
name: shared-test
packages:
  - packages/*
"#,
        )
        .unwrap();

        fs::create_dir_all(temp.path().join("packages")).unwrap();

        let result = shared_deps().await;

        if let Some(dir) = original_dir {
            let _ = env::set_current_dir(dir);
        }

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_shared_deps_with_shared_dependency() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().ok();
        env::set_current_dir(temp.path()).unwrap();

        fs::write(
            temp.path().join("workspace.yaml"),
            r#"
name: shared-workspace
packages:
  - packages/*
"#,
        )
        .unwrap();

        // Create two packages with shared dependency
        let pkg1 = temp.path().join("packages").join("pkg1");
        fs::create_dir_all(&pkg1).unwrap();
        fs::write(
            pkg1.join("package.yaml"),
            r#"
name: pkg1
version: 1.0.0
dependencies:
  inspect: "3.0.0"
"#,
        )
        .unwrap();

        let pkg2 = temp.path().join("packages").join("pkg2");
        fs::create_dir_all(&pkg2).unwrap();
        fs::write(
            pkg2.join("package.yaml"),
            r#"
name: pkg2
version: 1.0.0
dependencies:
  inspect: "3.0.0"
"#,
        )
        .unwrap();

        let result = shared_deps().await;

        if let Some(dir) = original_dir {
            let _ = env::set_current_dir(dir);
        }

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_shared_deps_version_conflict() {
        let temp = TempDir::new().unwrap();
        let original_dir = env::current_dir().ok();
        env::set_current_dir(temp.path()).unwrap();

        fs::write(
            temp.path().join("workspace.yaml"),
            r#"
name: conflict-workspace
packages:
  - packages/*
"#,
        )
        .unwrap();

        // Create packages with version conflicts
        let pkg1 = temp.path().join("packages").join("pkg1");
        fs::create_dir_all(&pkg1).unwrap();
        fs::write(
            pkg1.join("package.yaml"),
            r#"
name: pkg1
version: 1.0.0
dependencies:
  inspect: "3.0.0"
"#,
        )
        .unwrap();

        let pkg2 = temp.path().join("packages").join("pkg2");
        fs::create_dir_all(&pkg2).unwrap();
        fs::write(
            pkg2.join("package.yaml"),
            r#"
name: pkg2
version: 1.0.0
dependencies:
  inspect: "3.1.0"
"#,
        )
        .unwrap();

        let result = shared_deps().await;

        if let Some(dir) = original_dir {
            let _ = env::set_current_dir(dir);
        }

        assert!(result.is_ok());
    }
}
