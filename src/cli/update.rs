use depot::core::path::find_project_root;
use depot::core::{DepotError, DepotResult};
use depot::di::ServiceContainer;
use depot::package::installer::PackageInstaller;
use depot::package::lockfile::Lockfile;
use depot::package::lockfile_builder::LockfileBuilder;
use depot::package::manifest::PackageManifest;
use depot::path_setup::PathSetup;
use depot::workspace::{Workspace, WorkspaceFilter};
use std::collections::HashMap;
use std::env;

pub async fn run(package: Option<String>, filter: Vec<String>) -> DepotResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;

    // Check if we're in a workspace and filtering is requested
    if !filter.is_empty() {
        if Workspace::is_workspace(&project_root) {
            let workspace = Workspace::load(&project_root)?;
            return update_workspace_filtered(&workspace, &filter, package).await;
        } else {
            return Err(DepotError::Package(
                "--filter can only be used in workspace mode".to_string(),
            ));
        }
    }

    // Load current manifest and lockfile
    let manifest = PackageManifest::load(&project_root)?;
    let lockfile = Lockfile::load(&project_root)?;

    if let Some(ref pkg_name) = package {
        // Update specific package
        update_single_package(&project_root, &manifest, &lockfile, pkg_name).await?;
    } else {
        // Update all packages
        update_all_packages(&project_root, &manifest, &lockfile).await?;
    }

    Ok(())
}

async fn update_single_package(
    project_root: &std::path::Path,
    manifest: &PackageManifest,
    lockfile: &Option<Lockfile>,
    package_name: &str,
) -> DepotResult<()> {
    // Check if package exists in dependencies
    let _version_constraint = manifest
        .dependencies
        .get(package_name)
        .or_else(|| manifest.dev_dependencies.get(package_name))
        .ok_or_else(|| {
            DepotError::Package(format!(
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

    if let Some(current) = &current_version {
        println!("  Current version: {}", current);
    } else {
        println!("  Not currently installed");
    }

    // Initialize installer
    let container = ServiceContainer::new()?;
    let installer = PackageInstaller::new(
        project_root,
        container.cache.clone(),
        container.github.clone(),
        container.config.github_fallback_chain().to_vec(),
    )?;
    installer.init()?;

    // Remove old version if it exists
    if installer.is_installed(package_name) {
        installer.remove_package(package_name)?;
    }

    // Get the version spec from manifest
    let version_spec = manifest
        .dependencies
        .get(package_name)
        .or_else(|| manifest.dev_dependencies.get(package_name))
        .unwrap(); // Safe because we checked above

    // Install new version
    installer
        .install_package(package_name, Some(version_spec))
        .await?;

    println!("✓ Updated {}", package_name);

    // Regenerate lockfile
    let builder = LockfileBuilder::new(
        project_root,
        container.cache.clone(),
        container.github.clone(),
        container.config.github_fallback_chain().to_vec(),
    );
    let new_lockfile = builder.build(manifest).await?;
    new_lockfile.save(project_root)?;

    // Install loader
    PathSetup::install_loader(project_root)?;

    Ok(())
}

async fn update_all_packages(
    project_root: &std::path::Path,
    manifest: &PackageManifest,
    lockfile: &Option<Lockfile>,
) -> DepotResult<()> {
    println!("Updating all packages...");

    // Initialize installer
    let container = ServiceContainer::new()?;
    let installer = PackageInstaller::new(
        project_root,
        container.cache.clone(),
        container.github.clone(),
        container.config.github_fallback_chain().to_vec(),
    )?;
    installer.init()?;

    let mut updated_count = 0;

    // Collect all dependencies
    let mut all_deps = HashMap::new();
    all_deps.extend(manifest.dependencies.clone());
    all_deps.extend(manifest.dev_dependencies.clone());

    for (name, version_spec) in &all_deps {
        // Check current version from lockfile
        let current_version = lockfile
            .as_ref()
            .and_then(|lf| lf.get_package(name))
            .map(|pkg| pkg.version.clone());

        println!("  Updating {}...", name);

        // Remove old version if installed
        if installer.is_installed(name) {
            installer.remove_package(name)?;
        }

        // Install package
        installer.install_package(name, Some(version_spec)).await?;
        updated_count += 1;

        if let Some(current) = current_version {
            println!("    {} → latest", current);
        } else {
            println!("    → installed");
        }
    }

    println!("\n✓ Updated {} package(s)", updated_count);

    // Regenerate lockfile
    let builder = LockfileBuilder::new(
        project_root,
        container.cache.clone(),
        container.github.clone(),
        container.config.github_fallback_chain().to_vec(),
    );
    let new_lockfile = builder.build(manifest).await?;
    new_lockfile.save(project_root)?;

    // Install loader
    PathSetup::install_loader(project_root)?;

    Ok(())
}

async fn update_workspace_filtered(
    workspace: &Workspace,
    filter_patterns: &[String],
    package: Option<String>,
) -> DepotResult<()> {
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

        // Load package manifest
        let manifest = PackageManifest::load(&pkg_dir)?;
        let lockfile = Lockfile::load(&pkg_dir)?;

        if let Some(ref pkg_name) = package {
            // Update specific package in this workspace package
            update_single_package(&pkg_dir, &manifest, &lockfile, pkg_name).await?;
        } else {
            // Update all packages in this workspace package
            update_all_packages(&pkg_dir, &manifest, &lockfile).await?;
        }

        println!("✓ Updated dependencies for {}\n", pkg.name);
    }

    println!("✓ All filtered workspace packages updated");

    Ok(())
}
