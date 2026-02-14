use depot::core::path::find_project_root;
use depot::core::{DepotError, DepotResult};
use depot::di::ServiceContainer;
use depot::lua_version::detector::LuaVersionDetector;
use depot::package::conflict_checker::ConflictChecker;
use depot::package::installer::PackageInstaller;
use depot::package::lockfile_builder::LockfileBuilder;
use depot::package::manifest::PackageManifest;
use depot::package::rollback::with_rollback_async;
use depot::path_setup::loader::PathSetup;
use depot::workspace::{Workspace, WorkspaceFilter};
use dialoguer::{Confirm, Input};
use std::collections::HashMap;
use std::env;
use std::path::Path;

// Trait for user input (for dependency injection in tests)
pub trait UserInput {
    fn prompt_string(&self, prompt: &str) -> DepotResult<String>;
    fn prompt_confirm(&self, prompt: &str, default: bool) -> DepotResult<bool>;
}

// Real implementation using dialoguer
pub struct DialoguerInput;

impl UserInput for DialoguerInput {
    fn prompt_string(&self, prompt: &str) -> DepotResult<String> {
        Input::new()
            .with_prompt(prompt)
            .allow_empty(false)
            .interact_text()
            .map_err(|e| DepotError::Config(format!("Failed to read input: {}", e)))
    }

    fn prompt_confirm(&self, prompt: &str, default: bool) -> DepotResult<bool> {
        Confirm::new()
            .with_prompt(prompt)
            .default(default)
            .interact()
            .map_err(|e| DepotError::Config(format!("Failed to read input: {}", e)))
    }
}

pub struct InstallOptions {
    pub package: Option<String>,
    pub dev: bool,
    pub path: Option<String>,
    pub no_dev: bool,
    pub dev_only: bool,
    pub global: bool,
    pub interactive: bool,
    pub filter: Vec<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub release: Option<String>,
}

/// Parse package specification from either owner/repo[@version] or full GitHub URL
///
/// Supports:
/// - owner/repo[@version]
/// - https://github.com/owner/repo[@version]
/// - http://github.com/owner/repo[@version]
///
/// Returns (repository, version) where repository is always in "owner/repo" format
fn parse_package_spec(spec: &str) -> DepotResult<(String, Option<String>)> {
    let spec = spec.trim();

    // Check if it's a GitHub URL
    let repository_part = if spec.starts_with("https://github.com/") {
        spec.strip_prefix("https://github.com/").unwrap()
    } else if spec.starts_with("http://github.com/") {
        spec.strip_prefix("http://github.com/").unwrap()
    } else if spec.starts_with("github.com/") {
        spec.strip_prefix("github.com/").unwrap()
    } else {
        spec
    };

    // Remove trailing .git if present
    let repository_part = repository_part
        .strip_suffix(".git")
        .unwrap_or(repository_part);

    // Split on @ to get version
    let (repository, version) = if let Some(at_pos) = repository_part.find('@') {
        (
            repository_part[..at_pos].to_string(),
            Some(repository_part[at_pos + 1..].to_string()),
        )
    } else {
        (repository_part.to_string(), None)
    };

    // Validate repository format (must be owner/repo)
    let parts: Vec<&str> = repository.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(DepotError::Config(format!(
            "Invalid repository format '{}'. Expected 'owner/repo' or 'https://github.com/owner/repo'",
            spec
        )));
    }

    Ok((repository, version))
}

/// Determine the version string from package spec and ref flags
///
/// Priority: -c (commit) > -b (branch) > -r (release) > @version in spec > default
fn build_version_spec(
    parsed_version: Option<String>,
    branch: Option<String>,
    commit: Option<String>,
    release: Option<String>,
) -> String {
    if let Some(commit_sha) = commit {
        return commit_sha;
    }
    if let Some(branch_name) = branch {
        return branch_name;
    }
    if let Some(release_tag) = release {
        return release_tag;
    }
    parsed_version.unwrap_or_else(|| "*".to_string())
}

pub async fn run(options: InstallOptions) -> DepotResult<()> {
    let InstallOptions {
        package,
        dev,
        path,
        no_dev,
        dev_only,
        global,
        interactive,
        filter,
        branch,
        commit,
        release,
    } = options;
    // Validate conflicting flags early, before any other operations.
    if no_dev && dev_only {
        return Err(DepotError::Package(
            "Cannot use both --no-dev and --dev-only flags".to_string(),
        ));
    }

    // Handle global installation (install to system-wide location).
    if global {
        if package.is_none() {
            return Err(DepotError::Package(
                "Global installation requires a package name. Use: depot install -g <package>"
                    .to_string(),
            ));
        }
        if path.is_some() {
            return Err(DepotError::Package(
                "Cannot install from local path globally. Use: depot install --path <path>"
                    .to_string(),
            ));
        }

        let pkg_spec = package.unwrap();
        return install_global(&pkg_spec, branch, commit, release).await;
    }

    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;

    // Use rollback wrapper for safety
    with_rollback_async(&project_root, || async {
        // Check if we're in a workspace
        let workspace = if Workspace::is_workspace(&project_root) {
            Some(Workspace::load(&project_root)?)
        } else {
            None
        };

        // Determine installation root
        let install_root = if let Some(ref ws) = workspace {
            &ws.root
        } else {
            &project_root
        };

        // Handle --filter flag (workspace only)
        if !filter.is_empty() {
            if let Some(ref ws) = workspace {
                return install_workspace_filtered(ws, &filter, dev, no_dev, dev_only, package)
                    .await;
            } else {
                return Err(DepotError::Package(
                    "--filter can only be used in workspace mode".to_string(),
                ));
            }
        }

        // Handle --path flag (install from local path)
        if let Some(ref _local_path) = path {
            return Err(DepotError::NotImplemented(
                "Installing from local path is not yet implemented".to_string(),
            ));
        }

        // Load package manifest
        let mut manifest = PackageManifest::load(&project_root)?;

        // Validate that we don't have conflicting options
        if dev && dev_only {
            return Err(DepotError::Package(
                "Cannot use both --dev and --dev-only flags".to_string(),
            ));
        }

        // Check if there are any dependencies to install (for no-args case)
        if package.is_none() {
            let has_deps = !manifest.dependencies.is_empty()
                || (!no_dev && !manifest.dev_dependencies.is_empty())
                || (dev_only && !manifest.dev_dependencies.is_empty());

            if !has_deps {
                println!("No dependencies to install");
                return Ok(());
            }
        }

        // For operations that require Lua, detect and validate Lua version
        let installed_lua = LuaVersionDetector::detect()?;
        println!("Detected Lua version: {}", installed_lua.version_string());

        // Check for conflicts before installation
        ConflictChecker::check_conflicts(&manifest)?;

        // Handle interactive mode
        if interactive {
            return run_interactive(&project_root, dev, &mut manifest).await;
        }

        match package {
            // Install specific package
            Some(pkg_spec) => {
                // Parse owner/repo[@version] or full GitHub URL
                let (repository, parsed_version) = parse_package_spec(&pkg_spec)?;

                // Build version spec from flags and parsed version
                let version_str = build_version_spec(parsed_version, branch, commit, release);

                // Add to manifest
                if dev {
                    manifest
                        .dev_dependencies
                        .insert(repository.clone(), version_str.clone());
                    println!("Added {} to dev dependencies", repository);
                } else {
                    manifest
                        .dependencies
                        .insert(repository.clone(), version_str.clone());
                    println!("Added {} to dependencies", repository);
                }

                // Save manifest before installing
                manifest.save(&project_root)?;

                // Initialize installer
                let container = ServiceContainer::new()?;
                let installer = PackageInstaller::new(
                    &project_root,
                    container.cache.clone(),
                    container.github.clone(),
                    container.config.github_fallback_chain().to_vec(),
                )?;
                installer.init()?;

                // Install the package
                println!("Installing {}...", repository);
                installer
                    .install_package(&repository, Some(&version_str))
                    .await?;
                println!("✓ Installed {}", repository);

                // Generate loader
                PathSetup::install_loader(&project_root)?;

                // Generate lockfile
                generate_lockfile(&project_root, &manifest, no_dev).await?;

                return Ok(());
            }
            // Install all dependencies
            None => {
                if let Some(ref ws) = workspace {
                    // Install workspace dependencies (shared + all packages)
                    install_workspace_dependencies(install_root, ws, no_dev, dev_only).await?;
                } else {
                    // Install single package dependencies
                    install_package_dependencies(&project_root, &manifest, no_dev, dev_only)
                        .await?;
                }
            }
        }

        Ok(())
    })
    .await
}

/// Install dependencies for a single package (non-workspace)
async fn install_package_dependencies(
    project_root: &Path,
    manifest: &PackageManifest,
    no_dev: bool,
    dev_only: bool,
) -> DepotResult<()> {
    // Determine which dependencies to install
    let mut deps_to_install = HashMap::new();

    if !dev_only {
        deps_to_install.extend(manifest.dependencies.clone());
    }

    if !no_dev {
        deps_to_install.extend(manifest.dev_dependencies.clone());
    }

    if deps_to_install.is_empty() {
        println!("No dependencies to install");
        return Ok(());
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

    println!("Installing {} dependency(ies)...", deps_to_install.len());

    // Install all dependencies
    for (name, version) in &deps_to_install {
        println!("  Installing {}@{}...", name, version);
        installer.install_package(name, Some(version)).await?;
        println!("  ✓ Installed {}@{}", name, version);
    }

    // Generate loader
    PathSetup::install_loader(project_root)?;

    // Generate lockfile
    generate_lockfile(project_root, manifest, no_dev).await?;

    println!("\n✓ Installed {} package(s)", deps_to_install.len());

    Ok(())
}

async fn install_workspace_dependencies(
    install_root: &Path,
    workspace: &Workspace,
    no_dev: bool,
    dev_only: bool,
) -> DepotResult<()> {
    println!("Installing workspace dependencies...");

    // Initialize service container and package installer
    let container = ServiceContainer::new()?;
    let installer = PackageInstaller::new(
        install_root,
        container.cache.clone(),
        container.github.clone(),
        container.config.github_fallback_chain().to_vec(),
    )?;
    installer.init()?;

    // Collect all dependencies from workspace packages
    let mut all_dependencies = HashMap::new();
    let mut all_dev_dependencies = HashMap::new();

    // First, add workspace-level dependencies (inherited by all packages)
    if !dev_only {
        for (dep_name, dep_version) in workspace.workspace_dependencies() {
            all_dependencies.insert(dep_name.clone(), dep_version.clone());
        }
    }

    // Add workspace-level dev dependencies
    if !no_dev {
        for (dep_name, dep_version) in workspace.workspace_dev_dependencies() {
            all_dev_dependencies.insert(dep_name.clone(), dep_version.clone());
        }
    }

    // Then, collect dependencies from individual workspace packages
    for workspace_pkg in workspace.packages.values() {
        // Collect regular dependencies from workspace package
        if !dev_only {
            for (dep_name, dep_version) in &workspace_pkg.manifest.dependencies {
                // Package-level dependencies override workspace-level ones
                all_dependencies
                    .entry(dep_name.clone())
                    .or_insert_with(|| dep_version.clone());
            }
        }

        // Collect dev dependencies from workspace package
        if !no_dev {
            for (dep_name, dep_version) in &workspace_pkg.manifest.dev_dependencies {
                // Package-level dev dependencies override workspace-level ones
                all_dev_dependencies
                    .entry(dep_name.clone())
                    .or_insert_with(|| dep_version.clone());
            }
        }
    }

    let mut installed_count = 0;

    // Install regular dependencies
    for (name, version) in &all_dependencies {
        println!("  Installing {}@{} (shared)", name, version);
        installer.install_package(name, Some(version)).await?;
        installed_count += 1;
    }

    // Install dev dependencies
    if !no_dev {
        for (name, version) in &all_dev_dependencies {
            println!("  Installing {}@{} (shared, dev)", name, version);
            installer.install_package(name, Some(version)).await?;
            installed_count += 1;
        }
    }

    println!(
        "\n✓ Installed {} shared dependency(ies) at workspace root",
        installed_count
    );

    Ok(())
}

/// Generate lockfile from manifest
async fn generate_lockfile(
    project_root: &Path,
    manifest: &PackageManifest,
    no_dev: bool,
) -> DepotResult<()> {
    // Load service container
    let container = ServiceContainer::new()?;

    // Create lockfile builder with GitHub dependencies
    let builder = LockfileBuilder::new(
        project_root,
        container.cache.clone(),
        container.github.clone(),
        container.config.github_fallback_chain().to_vec(),
    );

    // Build lockfile from manifest
    let lockfile = builder.build(manifest).await?;

    // Save lockfile
    lockfile.save(project_root)?;

    println!("✓ Generated {}", depot::package::lockfile::LOCKFILE_NAME);
    if no_dev {
        println!("  (dev dependencies excluded)");
    }

    Ok(())
}

/// Interactive package installation
pub async fn run_interactive(
    project_root: &Path,
    dev: bool,
    manifest: &mut PackageManifest,
) -> DepotResult<()> {
    run_interactive_with_input(project_root, dev, manifest, &DialoguerInput).await
}

/// Interactive package installation with dependency injection
pub async fn run_interactive_with_input(
    project_root: &Path,
    dev: bool,
    manifest: &mut PackageManifest,
    input: &dyn UserInput,
) -> DepotResult<()> {
    println!("🔍 Interactive package installation");
    println!("Enter package repository in one of these formats:");
    println!("  - owner/repo[@version]");
    println!("  - https://github.com/owner/repo[@version]");
    println!("Example: lunarmodules/luasocket@3.0.0");
    println!();

    // Get package repository from user
    let package_spec = input.prompt_string("Package repository")?;

    // Parse owner/repo[@version] or full GitHub URL
    let (repository, version) = parse_package_spec(&package_spec)?;

    // Add to manifest
    let version_str = version.unwrap_or_else(|| "*".to_string());
    if dev {
        manifest
            .dev_dependencies
            .insert(repository.clone(), version_str.clone());
        println!("Added {} as dev dependency", repository);
    } else {
        manifest
            .dependencies
            .insert(repository.clone(), version_str.clone());
        println!("Added {} as dependency", repository);
    }

    // Ask if they want to install now
    let install_now = input.prompt_confirm("Install now?", true)?;

    if install_now {
        // Initialize installer
        let container = ServiceContainer::new()?;
        let installer = PackageInstaller::new(
            project_root,
            container.cache.clone(),
            container.github.clone(),
            container.config.github_fallback_chain().to_vec(),
        )?;
        installer.init()?;

        // Install the package
        println!("Installing {}...", repository);
        installer
            .install_package(&repository, version_str.as_str().into())
            .await?;
        println!("✓ Installed {}", repository);

        // Generate loader
        PathSetup::install_loader(project_root)?;

        // Generate lockfile
        generate_lockfile(project_root, manifest, false).await?;
    }

    Ok(())
}

async fn install_workspace_filtered(
    workspace: &Workspace,
    filter_patterns: &[String],
    dev: bool,
    no_dev: bool,
    dev_only: bool,
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
        "📦 Installing dependencies for {} workspace package(s):",
        filtered_packages.len()
    );
    for pkg in &filtered_packages {
        println!("  - {} ({})", pkg.name, pkg.path.display());
    }
    println!();

    // Install for each filtered package
    for pkg in filtered_packages {
        let pkg_dir = workspace.root.join(&pkg.path);

        println!("Installing dependencies for {}...", pkg.name);

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

        // If a specific package is requested, add it to this workspace package
        if let Some(ref pkg_spec) = package {
            // Parse package name and version
            let (pkg_name, version) = if let Some(at_pos) = pkg_spec.find('@') {
                (
                    pkg_spec[..at_pos].to_string(),
                    Some(pkg_spec[at_pos + 1..].to_string()),
                )
            } else {
                (pkg_spec.clone(), None)
            };

            let version_str = version.unwrap_or_else(|| "*".to_string());

            if dev {
                manifest.dev_dependencies.insert(pkg_name, version_str);
            } else {
                manifest.dependencies.insert(pkg_name, version_str);
            }

            manifest.save(&pkg_dir)?;
        }

        // Determine which dependencies to install
        let mut deps_to_install = HashMap::new();

        if !no_dev && !dev_only {
            // Install both prod and dev dependencies
            deps_to_install.extend(manifest.dependencies.clone());
            deps_to_install.extend(manifest.dev_dependencies.clone());
        } else if dev_only {
            // Install only dev dependencies
            deps_to_install.extend(manifest.dev_dependencies.clone());
        } else if no_dev {
            // Install only prod dependencies
            deps_to_install.extend(manifest.dependencies.clone());
        }

        if deps_to_install.is_empty() {
            println!("  No dependencies to install for {}", pkg.name);
            continue;
        }

        // Initialize installer for this package
        let container = ServiceContainer::new()?;
        let installer = PackageInstaller::new(
            &pkg_dir,
            container.cache.clone(),
            container.github.clone(),
            container.config.github_fallback_chain().to_vec(),
        )?;
        installer.init()?;

        // Install all dependencies
        for (dep_name, dep_version) in &deps_to_install {
            println!("  Installing {}@{}...", dep_name, dep_version);
            installer
                .install_package(dep_name, Some(dep_version))
                .await?;
            println!("  ✓ Installed {}@{}", dep_name, dep_version);
        }

        // Generate loader for this package
        PathSetup::install_loader(&pkg_dir)?;

        // Generate lockfile for this package
        generate_lockfile(&pkg_dir, &manifest, false).await?;

        println!("✓ Installed dependencies for {}\n", pkg.name);
    }

    println!("✓ All workspace packages updated");

    Ok(())
}

/// Install a package globally (system-wide)
async fn install_global(
    pkg_spec: &str,
    branch: Option<String>,
    commit: Option<String>,
    release: Option<String>,
) -> DepotResult<()> {
    use depot::config::Config;
    use depot::core::path::global_dir;
    use serde::Serialize;
    use std::fs;

    println!("Installing {} globally...", pkg_spec);

    // Parse package spec
    let (repository, parsed_version) = parse_package_spec(pkg_spec)?;
    let version_str = build_version_spec(parsed_version, branch, commit, release);

    // Load config to check for custom global path
    let config = Config::load().unwrap_or_default();
    let base_global_dir = if let Some(ref custom_path) = config.global_install_path {
        custom_path.clone()
    } else {
        global_dir()?
    };

    // Get global directories (using custom or default base)
    let global_lua_modules = base_global_dir.join("lua_modules");
    let global_bin = base_global_dir.join("bin");
    let metadata_dir = base_global_dir.join("packages");

    // Ensure directories exist
    fs::create_dir_all(&global_lua_modules)?;
    fs::create_dir_all(&global_bin)?;
    fs::create_dir_all(&metadata_dir)?;

    // Initialize installer with global directories
    let container = ServiceContainer::new()?;
    let installer = PackageInstaller::new(
        global_lua_modules.parent().unwrap(),
        container.cache.clone(),
        container.github.clone(),
        container.config.github_fallback_chain().to_vec(),
    )?;
    installer.init()?;

    // Install the package
    println!(
        "  Downloading and installing {}@{}...",
        repository, version_str
    );
    installer
        .install_package(&repository, Some(&version_str))
        .await?;

    // TODO: Extract executables and create shims in global_bin
    // For now, just save metadata

    // Save metadata
    #[derive(Serialize)]
    struct GlobalPackageMetadata {
        package: String,
        version: String,
        executables: Vec<String>,
    }

    let metadata = GlobalPackageMetadata {
        package: repository.clone(),
        version: version_str,
        executables: Vec::new(), // TODO: Scan for executables
    };

    let metadata_file = metadata_dir.join(format!("{}.yaml", repository.replace('/', "-")));
    let metadata_yaml = serde_yaml::to_string(&metadata)
        .map_err(|e| DepotError::Package(format!("Failed to serialize metadata: {}", e)))?;
    fs::write(&metadata_file, metadata_yaml)?;

    println!("✓ Installed {} globally", repository);
    println!(
        "  Location: {}",
        global_lua_modules.join(&repository).display()
    );

    Ok(())
}
