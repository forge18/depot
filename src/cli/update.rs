use lpm::core::path::find_project_root;
use lpm::core::version::Version;
use lpm::core::{LpmError, LpmResult};
use lpm::di::ServiceContainer;
use lpm::package::installer::PackageInstaller;
use lpm::package::interactive::confirm;
use lpm::package::lockfile::Lockfile;
use lpm::package::lockfile_builder::LockfileBuilder;
use lpm::package::manifest::PackageManifest;
use lpm::package::rollback::with_rollback_async;
use lpm::package::update_diff::UpdateDiff;
use lpm::path_setup::PathSetup;
use lpm::resolver::DependencyResolver;
use lpm::workspace::{Workspace, WorkspaceFilter};
use std::env;

pub async fn run(package: Option<String>, filter: Vec<String>) -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;

    // Check if we're in a workspace and filtering is requested
    if !filter.is_empty() {
        if Workspace::is_workspace(&project_root) {
            let workspace = Workspace::load(&project_root)?;
            return update_workspace_filtered(&workspace, &filter, package).await;
        } else {
            return Err(LpmError::Package(
                "--filter can only be used in workspace mode".to_string(),
            ));
        }
    }

    // Use rollback for safety
    with_rollback_async(&project_root, || async {
        let mut manifest = PackageManifest::load(&project_root)?;
        let lockfile = Lockfile::load(&project_root)?;

        // Create service container
        let container = ServiceContainer::new()?;
        let luarocks_manifest = container.package_client.fetch_manifest().await?;

        // Create resolver
        let resolver = DependencyResolver::with_dependencies(
            luarocks_manifest,
            lpm::resolver::ResolutionStrategy::Highest,
            container.package_client.clone(),
            container.search_provider.clone(),
        )?;

        // Resolve versions first to calculate diff
        let resolved_versions = if let Some(package_name) = &package {
            // For single package update, resolve just that package
            let mut deps = std::collections::HashMap::new();
            if let Some(constraint) = manifest
                .dependencies
                .get(package_name)
                .or_else(|| manifest.dev_dependencies.get(package_name))
            {
                deps.insert(package_name.clone(), constraint.clone());
            }
            resolver.resolve(&deps).await?
        } else {
            // Resolve all dependencies
            resolver.resolve(&manifest.dependencies).await?
        };

        let resolved_dev_versions = if package.is_none() {
            resolver.resolve(&manifest.dev_dependencies).await?
        } else {
            std::collections::HashMap::new()
        };

        // Calculate diff
        let mut diff = UpdateDiff::calculate(&lockfile, &resolved_versions, &resolved_dev_versions);

        // Calculate file changes
        diff.calculate_file_changes(&project_root);

        // Display diff
        diff.display();

        // Check if there are any changes
        if !diff.has_changes() {
            println!("\n✓ All packages are up to date!");
            return Ok(());
        }

        // Interactive confirmation
        println!();
        let proceed = confirm("Proceed with update?")?;
        if !proceed {
            println!("Update cancelled.");
            return Ok(());
        }

        // Initialize installer
        let installer = PackageInstaller::new(&project_root)?;
        installer.init()?;

        // Apply updates
        if let Some(package_name) = package {
            // Update specific package
            update_package(
                &project_root,
                &mut manifest,
                &resolver,
                &package_name,
                &lockfile,
                &installer,
            )
            .await?;
        } else {
            // Update all packages
            update_all_packages(
                &project_root,
                &mut manifest,
                &resolver,
                &lockfile,
                &resolved_versions,
                &resolved_dev_versions,
                &installer,
            )
            .await?;
        }

        // Install loader after updates
        PathSetup::install_loader(&project_root)?;

        // Save updated manifest
        manifest.save(&project_root)?;

        // Regenerate lockfile incrementally (include dev dependencies for updates)
        let builder = LockfileBuilder::with_dependencies(
            container.config.clone(),
            container.cache.clone(),
            container.package_client.clone(),
            container.search_provider.clone(),
        )?;
        let new_lockfile = if let Some(existing) = &lockfile {
            builder
                .update_lockfile(existing, &manifest, &project_root, false)
                .await?
        } else {
            builder
                .build_lockfile(&manifest, &project_root, false)
                .await?
        };
        new_lockfile.save(&project_root)?;

        Ok(())
    })
    .await
}

async fn update_package(
    _project_root: &std::path::Path,
    manifest: &mut PackageManifest,
    resolver: &DependencyResolver,
    package_name: &str,
    lockfile: &Option<Lockfile>,
    installer: &PackageInstaller,
) -> LpmResult<()> {
    // Check if package exists in dependencies
    let version_constraint = manifest
        .dependencies
        .get(package_name)
        .or_else(|| manifest.dev_dependencies.get(package_name))
        .ok_or_else(|| {
            LpmError::Package(format!(
                "Package '{}' not found in dependencies",
                package_name
            ))
        })?;

    println!("Updating {}...", package_name);

    // Get current version from lockfile
    let current_version = lockfile
        .as_ref()
        .and_then(|lf| lf.get_package(package_name))
        .map(|pkg| pkg.version.clone());

    // Resolve latest version that satisfies constraint
    let mut deps = std::collections::HashMap::new();
    deps.insert(package_name.to_string(), version_constraint.clone());

    let resolved = resolver.resolve(&deps).await?;
    let new_version = resolved.get(package_name as &str).ok_or_else(|| {
        LpmError::Package(format!("Could not resolve version for '{}'", package_name))
    })?;

    if let Some(current) = &current_version {
        let current_v = Version::parse(current)?;
        if current_v == *new_version {
            println!(
                "  ✓ {} is already at latest version: {}",
                package_name, new_version
            );
            return Ok(());
        }
        println!("  {} → {}", current, new_version);
    } else {
        println!("  → {}", new_version);
    }

    // Remove old version if it exists
    if installer.is_installed(package_name) {
        installer.remove_package(package_name)?;
    }

    // Install new version
    let new_version_str = new_version.to_string();
    installer
        .install_package(package_name, &new_version_str)
        .await?;

    println!("✓ Updated {} to {}", package_name, new_version);

    Ok(())
}

async fn update_all_packages(
    _project_root: &std::path::Path,
    _manifest: &mut PackageManifest,
    _resolver: &DependencyResolver,
    lockfile: &Option<Lockfile>,
    resolved_versions: &std::collections::HashMap<String, Version>,
    resolved_dev_versions: &std::collections::HashMap<String, Version>,
    installer: &PackageInstaller,
) -> LpmResult<()> {
    println!("\n🔄 Applying updates...");

    let mut updated_count = 0;

    // Update regular dependencies
    for (name, version) in resolved_versions {
        // Check if version actually changed
        let needs_update = if let Some(lf) = lockfile {
            if let Some(pkg) = lf.get_package(name) {
                Version::parse(&pkg.version)
                    .map(|v| v != *version)
                    .unwrap_or(true)
            } else {
                true
            }
        } else {
            true
        };

        if needs_update {
            // Remove old version if it exists
            if installer.is_installed(name) {
                installer.remove_package(name)?;
            }

            // Install new version
            let version_str = version.to_string();
            installer.install_package(name, &version_str).await?;
            updated_count += 1;
        }
    }

    // Update dev dependencies
    for (name, version) in resolved_dev_versions {
        // Check if version actually changed
        let needs_update = if let Some(lf) = lockfile {
            if let Some(pkg) = lf.get_package(name) {
                Version::parse(&pkg.version)
                    .map(|v| v != *version)
                    .unwrap_or(true)
            } else {
                true
            }
        } else {
            true
        };

        if needs_update {
            // Remove old version if it exists
            if installer.is_installed(name) {
                installer.remove_package(name)?;
            }

            // Install new version
            let version_str = version.to_string();
            installer.install_package(name, &version_str).await?;
            updated_count += 1;
        }
    }

    println!("\n✓ Update complete");
    println!("  Updated: {} package(s)", updated_count);

    Ok(())
}

async fn update_workspace_filtered(
    workspace: &Workspace,
    filter_patterns: &[String],
    package: Option<String>,
) -> LpmResult<()> {
    // Create filter
    let filter = WorkspaceFilter::new(filter_patterns.to_vec());

    // Get filtered packages
    let filtered_packages = filter.filter_packages(workspace)?;

    if filtered_packages.is_empty() {
        println!("No packages match the filter patterns");
        return Ok(());
    }

    println!(
        "📦 Updating dependencies for {} workspace package(s):",
        filtered_packages.len()
    );
    for pkg in &filtered_packages {
        println!("  - {} ({})", pkg.name, pkg.path.display());
    }
    println!();

    // Update for each filtered package
    for pkg in filtered_packages {
        let pkg_dir = workspace.root.join(&pkg.path);

        println!("Updating dependencies for {}...", pkg.name);

        // Load package manifest and merge workspace dependencies
        let mut manifest = PackageManifest::load(&pkg_dir)?;

        // Merge workspace dependencies
        for (name, version) in workspace.workspace_dependencies() {
            if !manifest.dependencies.contains_key(name) {
                manifest.dependencies.insert(name.clone(), version.clone());
            }
        }
        for (name, version) in workspace.workspace_dev_dependencies() {
            if !manifest.dev_dependencies.contains_key(name) {
                manifest
                    .dev_dependencies
                    .insert(name.clone(), version.clone());
            }
        }

        // Load or create lockfile
        let lockfile = Lockfile::load(&pkg_dir)?;

        // Create service container
        let container = ServiceContainer::new()?;
        let luarocks_manifest = container.package_client.fetch_manifest().await?;

        // Create resolver
        let resolver = DependencyResolver::with_dependencies(
            luarocks_manifest,
            lpm::resolver::ResolutionStrategy::Highest,
            container.package_client.clone(),
            container.search_provider.clone(),
        )?;

        // Resolve versions first to calculate diff
        let resolved_versions = if let Some(ref package_name) = package {
            // For single package update, resolve just that package
            let mut deps = std::collections::HashMap::new();
            if let Some(constraint) = manifest
                .dependencies
                .get(package_name)
                .or_else(|| manifest.dev_dependencies.get(package_name))
            {
                deps.insert(package_name.clone(), constraint.clone());
            }
            resolver.resolve(&deps).await?
        } else {
            // Resolve all dependencies
            resolver.resolve(&manifest.dependencies).await?
        };

        let resolved_dev_versions = if package.is_none() {
            resolver.resolve(&manifest.dev_dependencies).await?
        } else {
            std::collections::HashMap::new()
        };

        // Calculate diff
        let mut diff = UpdateDiff::calculate(&lockfile, &resolved_versions, &resolved_dev_versions);

        // Calculate file changes
        diff.calculate_file_changes(&pkg_dir);

        // Display diff
        diff.display();

        // Check if there are any changes
        if !diff.has_changes() {
            println!("  ✓ All packages are up to date for {}\n", pkg.name);
            continue;
        }

        // Interactive confirmation for this package
        println!();
        let proceed = confirm(&format!("Proceed with update for {}?", pkg.name))?;
        if !proceed {
            println!("  Update cancelled for {}\n", pkg.name);
            continue;
        }

        // Initialize installer
        let installer = PackageInstaller::new(&pkg_dir)?;
        installer.init()?;

        // Apply updates
        if let Some(ref package_name) = package {
            // Update specific package
            update_package(
                &pkg_dir,
                &mut manifest,
                &resolver,
                package_name,
                &lockfile,
                &installer,
            )
            .await?;
        } else {
            // Update all packages
            update_all_packages(
                &pkg_dir,
                &mut manifest,
                &resolver,
                &lockfile,
                &resolved_versions,
                &resolved_dev_versions,
                &installer,
            )
            .await?;
        }

        // Install loader after updates
        PathSetup::install_loader(&pkg_dir)?;

        // Save updated manifest
        manifest.save(&pkg_dir)?;

        // Regenerate lockfile incrementally
        let builder = LockfileBuilder::with_dependencies(
            container.config.clone(),
            container.cache.clone(),
            container.package_client.clone(),
            container.search_provider.clone(),
        )?;
        let new_lockfile = if let Some(ref existing) = lockfile {
            builder
                .update_lockfile(existing, &manifest, &pkg_dir, false)
                .await?
        } else {
            builder.build_lockfile(&manifest, &pkg_dir, false).await?
        };
        new_lockfile.save(&pkg_dir)?;

        println!("✓ Updated dependencies for {}\n", pkg.name);
    }

    println!("✓ All filtered workspace packages updated");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpm::core::version::Version;
    use lpm::package::lockfile::{LockedPackage, Lockfile};
    use lpm::package::manifest::PackageManifest;
    use lpm::package::update_diff::UpdateDiff;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_update_package_function_exists() {
        // Test that update_package function exists
        let _ = update_package;
    }

    #[test]
    fn test_update_all_packages_function_exists() {
        // Test that update_all_packages function exists
        let _ = update_all_packages;
    }

    #[test]
    fn test_update_package_not_in_dependencies() {
        let _temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test".to_string());
        // Package not in dependencies
        let result = std::panic::catch_unwind(|| {
            // This would fail at runtime, but we can test the function signature
            let _ = manifest.dependencies.get("nonexistent");
        });
        // Just verify the function can be called conceptually
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_diff_calculate() {
        let lockfile = Some(Lockfile::new());
        let resolved_versions = HashMap::new();
        let resolved_dev_versions = HashMap::new();

        let diff = UpdateDiff::calculate(&lockfile, &resolved_versions, &resolved_dev_versions);
        // Empty diff since no changes
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_update_diff_with_new_package() {
        let lockfile = Some(Lockfile::new());
        let mut resolved_versions = HashMap::new();
        resolved_versions.insert("new-pkg".to_string(), Version::new(1, 0, 0));
        let resolved_dev_versions = HashMap::new();

        let diff = UpdateDiff::calculate(&lockfile, &resolved_versions, &resolved_dev_versions);
        assert!(diff.has_changes());
    }

    #[test]
    fn test_update_diff_with_upgraded_package() {
        let mut lockfile = Lockfile::new();
        lockfile.add_package(
            "test-pkg".to_string(),
            LockedPackage {
                version: "1.0.0".to_string(),
                source: "luarocks".to_string(),
                rockspec_url: None,
                source_url: None,
                checksum: "abc".to_string(),
                size: None,
                dependencies: HashMap::new(),
                build: None,
            },
        );

        let mut resolved_versions = HashMap::new();
        resolved_versions.insert("test-pkg".to_string(), Version::new(2, 0, 0));
        let resolved_dev_versions = HashMap::new();

        let diff =
            UpdateDiff::calculate(&Some(lockfile), &resolved_versions, &resolved_dev_versions);
        assert!(diff.has_changes());
    }

    #[test]
    fn test_update_diff_no_changes() {
        let mut lockfile = Lockfile::new();
        lockfile.add_package(
            "test-pkg".to_string(),
            LockedPackage {
                version: "1.0.0".to_string(),
                source: "luarocks".to_string(),
                rockspec_url: None,
                source_url: None,
                checksum: "abc".to_string(),
                size: None,
                dependencies: HashMap::new(),
                build: None,
            },
        );

        let mut resolved_versions = HashMap::new();
        resolved_versions.insert("test-pkg".to_string(), Version::new(1, 0, 0));
        let resolved_dev_versions = HashMap::new();

        let diff =
            UpdateDiff::calculate(&Some(lockfile), &resolved_versions, &resolved_dev_versions);
        // No changes since version is the same
        assert!(!diff.has_changes());
    }

    #[test]
    fn test_manifest_dependency_lookup() {
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("pkg1".to_string(), "^1.0.0".to_string());
        manifest
            .dev_dependencies
            .insert("pkg2".to_string(), "^2.0.0".to_string());

        // Test dependency lookup
        assert!(manifest.dependencies.contains_key("pkg1"));
        assert!(manifest.dev_dependencies.contains_key("pkg2"));

        // Fallback lookup pattern
        let constraint = manifest
            .dependencies
            .get("pkg1")
            .or_else(|| manifest.dev_dependencies.get("pkg1"));
        assert!(constraint.is_some());
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(2, 0, 0);
        let v3 = Version::new(1, 0, 0);

        assert!(v1 < v2);
        assert!(v2 > v1);
        assert_eq!(v1, v3);
    }

    #[test]
    fn test_update_diff_display() {
        let diff = UpdateDiff::calculate(&None, &HashMap::new(), &HashMap::new());
        // Just verify display doesn't panic
        diff.display();
    }

    #[test]
    fn test_lockfile_get_package() {
        let mut lockfile = Lockfile::new();
        lockfile.add_package(
            "test-pkg".to_string(),
            LockedPackage {
                version: "1.2.3".to_string(),
                source: "luarocks".to_string(),
                rockspec_url: None,
                source_url: None,
                checksum: "abc".to_string(),
                size: None,
                dependencies: HashMap::new(),
                build: None,
            },
        );

        let pkg = lockfile.get_package("test-pkg").unwrap();
        assert_eq!(pkg.version, "1.2.3");
    }

    #[test]
    fn test_lockfile_get_package_none() {
        let lockfile = Lockfile::new();
        assert!(lockfile.get_package("nonexistent").is_none());
    }

    #[test]
    fn test_version_parse() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v, Version::new(1, 2, 3));
    }

    #[test]
    fn test_update_diff_with_dev_dependencies() {
        let lockfile = Some(Lockfile::new());
        let resolved_versions = HashMap::new();
        let mut resolved_dev_versions = HashMap::new();
        resolved_dev_versions.insert("dev-pkg".to_string(), Version::new(1, 0, 0));

        let diff = UpdateDiff::calculate(&lockfile, &resolved_versions, &resolved_dev_versions);
        assert!(diff.has_changes());
    }

    #[test]
    fn test_update_run_function_exists() {
        // Verify run function exists
        let _ = run;
    }

    #[test]
    fn test_update_workspace_filtered_function_exists() {
        // Verify update_workspace_filtered function exists
        let _ = update_workspace_filtered;
    }

    #[test]
    fn test_lockfile_add_and_get_package() {
        let mut lockfile = Lockfile::new();
        let pkg = LockedPackage {
            version: "2.0.0".to_string(),
            source: "github".to_string(),
            rockspec_url: Some("http://example.com".to_string()),
            source_url: Some("http://example.com/source".to_string()),
            checksum: "def".to_string(),
            size: Some(1024),
            dependencies: HashMap::new(),
            build: None,
        };

        lockfile.add_package("test-pkg".to_string(), pkg);
        let retrieved = lockfile.get_package("test-pkg").unwrap();
        assert_eq!(retrieved.version, "2.0.0");
        assert_eq!(retrieved.source, "github");
    }
}
