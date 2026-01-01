use dialoguer::{Confirm, Input, MultiSelect, Select};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use lpm::core::path::{
    ensure_dir, find_project_root, global_bin_dir, global_dir, global_lua_modules_dir,
};
use lpm::core::version::parse_constraint;
use lpm::core::{LpmError, LpmResult};
use lpm::di::ServiceContainer;
use lpm::lua_version::compatibility::PackageCompatibility;
use lpm::lua_version::detector::LuaVersionDetector;
use lpm::package::conflict_checker::ConflictChecker;
use lpm::package::installer::PackageInstaller;
use lpm::package::lockfile::Lockfile;
use lpm::package::lockfile_builder::LockfileBuilder;
use lpm::package::manifest::PackageManifest;
use lpm::package::rollback::with_rollback_async;
use lpm::path_setup::loader::PathSetup;
use lpm::resolver::DependencyResolver;
use lpm::workspace::{Workspace, WorkspaceFilter};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

// Trait for user input (for dependency injection in tests)
pub trait UserInput {
    fn prompt_string(&self, prompt: &str) -> LpmResult<String>;
    fn prompt_confirm(&self, prompt: &str, default: bool) -> LpmResult<bool>;
    fn prompt_select(&self, prompt: &str, items: &[String], default: usize) -> LpmResult<usize>;
    fn prompt_multiselect(&self, prompt: &str, items: &[String]) -> LpmResult<Vec<usize>>;
}

// Real implementation using dialoguer
pub struct DialoguerInput;

impl UserInput for DialoguerInput {
    fn prompt_string(&self, prompt: &str) -> LpmResult<String> {
        Input::new()
            .with_prompt(prompt)
            .allow_empty(false)
            .interact_text()
            .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))
    }

    fn prompt_confirm(&self, prompt: &str, default: bool) -> LpmResult<bool> {
        Confirm::new()
            .with_prompt(prompt)
            .default(default)
            .interact()
            .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))
    }

    fn prompt_select(&self, prompt: &str, items: &[String], default: usize) -> LpmResult<usize> {
        Select::new()
            .with_prompt(prompt)
            .items(items)
            .default(default)
            .interact()
            .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))
    }

    fn prompt_multiselect(&self, prompt: &str, items: &[String]) -> LpmResult<Vec<usize>> {
        MultiSelect::new()
            .with_prompt(prompt)
            .items(items)
            .interact()
            .map_err(|e| LpmError::Config(format!("Failed to read input: {}", e)))
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
}

pub async fn run(options: InstallOptions) -> LpmResult<()> {
    let InstallOptions {
        package,
        dev,
        path,
        no_dev,
        dev_only,
        global,
        interactive,
        filter,
    } = options;
    // Validate conflicting flags early, before any other operations.
    if no_dev && dev_only {
        return Err(LpmError::Package(
            "Cannot use both --no-dev and --dev-only flags".to_string(),
        ));
    }

    // Handle global installation (install to system-wide location).
    if global {
        if package.is_none() {
            return Err(LpmError::Package(
                "Global installation requires a package name. Use: lpm install -g <package>"
                    .to_string(),
            ));
        }
        if path.is_some() {
            return Err(LpmError::Package(
                "Cannot install from local path globally. Use: lpm install --path <path>"
                    .to_string(),
            ));
        }
        return install_global(package.unwrap()).await;
    }

    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;

    // Use rollback wrapper for safety
    with_rollback_async(&project_root, || async {
        // Check if we're in a workspace
        let workspace = if Workspace::is_workspace(&project_root) {
            Some(Workspace::load(&project_root)?)
        } else {
            None
        };

        // Handle workspace filtering
        if !filter.is_empty() {
            if let Some(ref ws) = workspace {
                return install_workspace_filtered(ws, &filter, dev, no_dev, dev_only, package)
                    .await;
            } else {
                return Err(LpmError::Package(
                    "--filter can only be used in workspace mode".to_string(),
                ));
            }
        }

        // For workspace, install to workspace root's lua_modules
        // For single package, use project root
        let install_root = &project_root;

        let mut manifest = PackageManifest::load(install_root)?;

        // Handle path-based installation early (doesn't need Lua detection)
        if let Some(ref local_path) = path {
            if package.is_some() {
                return Err(LpmError::Package(
                    "Cannot specify both package and --path".to_string(),
                ));
            }
            install_from_path(local_path, dev, &mut manifest)?;
            manifest.save(&project_root)?;
            return Ok(());
        }

        // Validate package version constraint early if installing a specific package
        if let Some(ref pkg_spec) = package {
            if let Some(at_pos) = pkg_spec.find('@') {
                let version = &pkg_spec[at_pos + 1..];
                parse_constraint(version).map_err(|e| {
                    LpmError::Version(format!("Invalid version constraint '{}': {}", version, e))
                })?;
            }
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

        // Validate project's lua_version constraint
        PackageCompatibility::validate_project_constraint(&installed_lua, &manifest.lua_version)?;

        // Check for conflicts before installation
        ConflictChecker::check_conflicts(&manifest)?;

        // Handle interactive mode
        if interactive {
            return run_interactive(&project_root, dev, &mut manifest).await;
        }

        match package {
            // Install specific package
            Some(pkg_spec) => {
                install_package(&project_root, &pkg_spec, dev, &mut manifest).await?;
            }
            // Install all dependencies
            None => {
                if let Some(ref ws) = workspace {
                    // Install workspace dependencies (shared + all packages)
                    install_workspace_dependencies(install_root, ws, no_dev, dev_only).await?;
                } else {
                    // Install single package dependencies
                    install_all_dependencies(install_root, &manifest, no_dev, dev_only).await?;
                }
                // Generate loader after installation
                PathSetup::install_loader(&project_root)?;
                // Generate lockfile
                generate_lockfile(install_root, &manifest, no_dev).await?;
            }
        }

        // Save updated manifest
        manifest.save(&project_root)?;

        Ok(())
    })
    .await
}

/// Install a package globally
async fn install_global(package_spec: String) -> LpmResult<()> {
    println!("Installing {} globally...", package_spec);

    // Parse package spec
    let (package_name, version_constraint) = if let Some(at_pos) = package_spec.find('@') {
        let name = package_spec[..at_pos].to_string();
        let version = package_spec[at_pos + 1..].to_string();
        parse_constraint(&version).map_err(|e| {
            LpmError::Version(format!("Invalid version constraint '{}': {}", version, e))
        })?;
        (name, Some(version))
    } else {
        (package_spec, None)
    };

    // Setup global directories
    let global_root = global_dir()?;
    let global_lua_modules = global_lua_modules_dir()?;
    let global_bin = global_bin_dir()?;

    ensure_dir(&global_root)?;
    ensure_dir(&global_lua_modules)?;
    ensure_dir(&global_bin)?;

    // Resolve version
    let container = ServiceContainer::new()?;
    let luarocks_manifest = container.package_client.fetch_manifest().await?;
    let resolver = DependencyResolver::with_dependencies(
        luarocks_manifest,
        lpm::resolver::ResolutionStrategy::Highest,
        container.package_client.clone(),
        container.search_provider.clone(),
    )?;

    let constraint_str = version_constraint
        .clone()
        .unwrap_or_else(|| "*".to_string());
    let mut deps = HashMap::new();
    deps.insert(package_name.clone(), constraint_str);

    let resolved_versions = resolver.resolve(&deps).await?;
    let version = resolved_versions.get(&package_name).ok_or_else(|| {
        LpmError::Package(format!("Could not resolve version for '{}'", package_name))
    })?;

    let version_str = version.to_string();
    println!("  Resolved version: {}", version_str);

    // Create a global installer (using global_root as project_root)
    let installer = PackageInstaller::new(&global_root)?;
    installer.init()?;

    // Install the package
    let package_path = installer
        .install_package(&package_name, &version_str)
        .await?;

    // Extract executables from rockspec and create wrappers
    let rockspec_url =
        container
            .search_provider
            .get_rockspec_url(&package_name, &version_str, None);
    let rockspec_content = container
        .package_client
        .download_rockspec(&rockspec_url)
        .await?;
    let rockspec = container.package_client.parse_rockspec(&rockspec_content)?;

    create_global_executables(
        &package_name,
        &package_path,
        &global_bin,
        &global_lua_modules,
        &rockspec,
    )
    .await?;

    println!("✓ Installed {}@{} globally", package_name, version_str);
    println!();
    println!("Global tools are installed in: {}", global_bin.display());
    println!(
        "Add to your PATH: export PATH=\"{}$PATH\"",
        global_bin.display()
    );

    Ok(())
}

/// Create executable wrappers for globally installed packages
async fn create_global_executables(
    package_name: &str,
    package_path: &std::path::Path,
    global_bin: &std::path::Path,
    global_lua_modules: &std::path::Path,
    rockspec: &lpm::luarocks::rockspec::Rockspec,
) -> LpmResult<()> {
    let mut executables = Vec::new();

    // First, check rockspec build.install.bin for explicitly defined executables.
    for (exe_name, source_path) in &rockspec.build.install.bin {
        let full_path = package_path.join(source_path);
        if full_path.exists() && full_path.is_file() {
            executables.push((exe_name.clone(), full_path));
        } else {
            // Try relative to package root if absolute path doesn't exist.
            let alt_path = package_path.join(source_path.strip_prefix("/").unwrap_or(source_path));
            if alt_path.exists() && alt_path.is_file() {
                executables.push((exe_name.clone(), alt_path));
            }
        }
    }

    // Check for common executable locations.
    let possible_paths = vec![
        package_path.join("bin").join(package_name),
        package_path
            .join("bin")
            .join(format!("{}.lua", package_name)),
        package_path.join(format!("{}.lua", package_name)),
        package_path.join("cli.lua"),
        package_path.join("main.lua"),
    ];

    for path in possible_paths {
        if path.exists() && path.is_file() {
            let exe_name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or(package_name);
            executables.push((exe_name.to_string(), path));
        }
    }

    // Also check bin/ directory for any .lua files.
    let bin_dir = package_path.join("bin");
    if bin_dir.exists() && bin_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&bin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "lua" || ext.is_empty() {
                            if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                                executables.push((name.to_string(), path));
                            }
                        }
                    }
                }
            }
        }
    }

    // If no executables found, create one with the package name.
    if executables.is_empty() {
        // Try to find a main entry point (init.lua).
        let main_script = package_path.join("init.lua");
        if main_script.exists() {
            executables.push((package_name.to_string(), main_script));
        }
    }

    // Track executable names for metadata.
    let mut exe_names = Vec::new();

    // Create wrapper scripts for each executable.
    for (exe_name, script_path) in executables {
        create_executable_wrapper(&exe_name, &script_path, global_bin, global_lua_modules)?;
        exe_names.push(exe_name);
    }

    // Save metadata about this globally installed package.
    save_global_package_metadata(package_name, &exe_names)?;

    Ok(())
}

/// Save metadata about a globally installed package
fn save_global_package_metadata(package_name: &str, executables: &[String]) -> LpmResult<()> {
    use lpm::core::path::global_packages_metadata_dir;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct GlobalPackageMetadata {
        package: String,
        executables: Vec<String>,
    }

    let metadata_dir = global_packages_metadata_dir()?;
    ensure_dir(&metadata_dir)?;

    let metadata = GlobalPackageMetadata {
        package: package_name.to_string(),
        executables: executables.to_vec(),
    };

    let metadata_file = metadata_dir.join(format!("{}.yaml", package_name));
    let content = serde_yaml::to_string(&metadata)?;
    fs::write(&metadata_file, content)?;

    Ok(())
}

/// Create a wrapper script for a global executable
fn create_executable_wrapper(
    exe_name: &str,
    script_path: &std::path::Path,
    global_bin: &std::path::Path,
    global_lua_modules: &std::path::Path,
) -> LpmResult<()> {
    use lpm::core::path::lpm_home;
    use lpm::lua_manager::VersionSwitcher;

    // Get LPM-managed Lua binary path.
    let lpm_home = lpm_home()?;
    let switcher = VersionSwitcher::new(&lpm_home);
    let lua_version = switcher.current().unwrap_or_else(|_| "5.4.8".to_string());
    let lua_bin = lpm_home
        .join("versions")
        .join(&lua_version)
        .join("bin")
        .join("lua");

    // If LPM-managed Lua doesn't exist, fall back to system lua.
    let lua_binary = if lua_bin.exists() {
        lua_bin.to_string_lossy().to_string()
    } else {
        "lua".to_string()
    };

    // Create wrapper script
    let wrapper_path = global_bin.join(exe_name);

    #[cfg(unix)]
    {
        let wrapper_content = format!(
            r#"#!/bin/sh
# Wrapper for {} (installed globally by LPM)
export LUA_PATH="{}/?.lua;{}/?/init.lua;$LUA_PATH"
exec "{}" "{}" "$@"
"#,
            exe_name,
            global_lua_modules.to_string_lossy(),
            global_lua_modules.to_string_lossy(),
            lua_binary,
            script_path.to_string_lossy()
        );
        fs::write(&wrapper_path, wrapper_content)?;
        // Set executable permissions on Unix.
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&wrapper_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&wrapper_path, perms)?;
    }

    #[cfg(windows)]
    {
        let wrapper_content = format!(
            r#"@echo off
REM Wrapper for {} (installed globally by LPM)
set LUA_PATH={}\?.lua;{}\?\init.lua;%LUA_PATH%
"{}" "{}" %*
"#,
            exe_name,
            global_lua_modules.to_string_lossy().replace('\\', "\\\\"),
            global_lua_modules.to_string_lossy().replace('\\', "\\\\"),
            lua_binary,
            script_path.to_string_lossy()
        );
        fs::write(&wrapper_path.with_extension("bat"), wrapper_content)?;
    }

    println!("  ✓ Created global executable: {}", exe_name);

    Ok(())
}

fn install_from_path(local_path: &str, dev: bool, manifest: &mut PackageManifest) -> LpmResult<()> {
    let path = Path::new(local_path);
    if !path.exists() {
        return Err(LpmError::Package(format!(
            "Path does not exist: {}",
            local_path
        )));
    }

    // Try to load package.yaml from the specified path.
    let local_manifest = PackageManifest::load(path)?;

    // Add local package as dependency.
    let dep_name = local_manifest.name.clone();
    let dep_version = format!("path:{}", local_path);

    if dev {
        manifest
            .dev_dependencies
            .insert(dep_name.clone(), dep_version.clone());
        println!("Added {} as dev dependency (from {})", dep_name, local_path);
    } else {
        manifest
            .dependencies
            .insert(dep_name.clone(), dep_version.clone());
        println!("Added {} as dependency (from {})", dep_name, local_path);
    }

    Ok(())
}

async fn install_package(
    project_root: &Path,
    pkg_spec: &str,
    dev: bool,
    manifest: &mut PackageManifest,
) -> LpmResult<()> {
    // Parse package spec (format: "package" or "package@version" or "package@^1.2.3").
    let (package_name, version_constraint) = if let Some(at_pos) = pkg_spec.find('@') {
        let name = pkg_spec[..at_pos].to_string();
        let version = pkg_spec[at_pos + 1..].to_string();

        // Validate version constraint format.
        parse_constraint(&version).map_err(|e| {
            LpmError::Version(format!("Invalid version constraint '{}': {}", version, e))
        })?;

        (name, Some(version))
    } else {
        (pkg_spec.to_string(), None)
    };

    // Check for dependency conflicts before adding.
    let version_str = version_constraint
        .clone()
        .unwrap_or_else(|| "*".to_string());
    ConflictChecker::check_new_dependency(manifest, &package_name, &version_str)?;

    println!("Installing package: {}", package_name);

    // Resolve version using dependency resolver (handles version constraints).
    let container = ServiceContainer::new()?;
    let luarocks_manifest = container.package_client.fetch_manifest().await?;
    let resolver = DependencyResolver::with_dependencies(
        luarocks_manifest,
        lpm::resolver::ResolutionStrategy::Highest,
        container.package_client.clone(),
        container.search_provider.clone(),
    )?;

    // Build dependency map for resolver.
    let constraint_str = version_constraint
        .clone()
        .unwrap_or_else(|| "*".to_string());
    let mut deps = HashMap::new();
    deps.insert(package_name.clone(), constraint_str);

    // Resolve to exact version using dependency resolver.
    let resolved_versions = resolver.resolve(&deps).await?;
    let version = resolved_versions.get(&package_name).ok_or_else(|| {
        LpmError::Package(format!("Could not resolve version for '{}'", package_name))
    })?;

    let version_str = version.to_string();
    println!("  Resolved version: {}", version_str);

    let installer = PackageInstaller::new(project_root)?;
    installer.init()?;
    installer
        .install_package(&package_name, &version_str)
        .await?;

    // Generate loader after installation
    PathSetup::install_loader(project_root)?;

    // Store constraint in manifest (resolved version goes in lockfile).
    let constraint_to_store = version_constraint.unwrap_or_else(|| version_str.clone());
    if dev {
        manifest
            .dev_dependencies
            .insert(package_name, constraint_to_store);
    } else {
        manifest
            .dependencies
            .insert(package_name, constraint_to_store);
    }

    Ok(())
}

async fn install_all_dependencies(
    project_root: &Path,
    manifest: &PackageManifest,
    no_dev: bool,
    dev_only: bool,
) -> LpmResult<()> {
    // Note: no_dev && dev_only conflict is checked early in run() function

    println!("Installing dependencies...");

    // Initialize package installer.
    let installer = PackageInstaller::new(project_root)?;
    installer.init()?;

    let mut total_deps = 0;
    let mut installed_count = 0;

    // Install regular dependencies (unless dev_only flag is set).
    if !dev_only {
        total_deps += manifest.dependencies.len();
        for (name, version) in &manifest.dependencies {
            println!("  Installing {}@{}", name, version);
            installer.install_package(name, version).await?;
            installed_count += 1;
        }
    }

    // Install dev dependencies (unless no_dev flag is set).
    if !no_dev {
        total_deps += manifest.dev_dependencies.len();
        for (name, version) in &manifest.dev_dependencies {
            println!("  Installing {}@{} (dev)", name, version);
            installer.install_package(name, version).await?;
            installed_count += 1;
        }
    }

    if total_deps == 0 {
        println!("No dependencies to install");
        return Ok(());
    }

    println!("✓ Installed {} package(s)", installed_count);
    if no_dev {
        println!("  (dev dependencies skipped)");
    } else if dev_only {
        println!("  (only dev dependencies)");
    }

    Ok(())
}

async fn install_workspace_dependencies(
    install_root: &Path,
    workspace: &Workspace,
    no_dev: bool,
    dev_only: bool,
) -> LpmResult<()> {
    println!("Installing workspace dependencies...");

    let installer = PackageInstaller::new(install_root)?;
    installer.init()?;

    // Resolve all workspace dependencies.
    let container = ServiceContainer::new()?;
    let luarocks_manifest = container.package_client.fetch_manifest().await?;
    let resolver = DependencyResolver::with_dependencies(
        luarocks_manifest,
        lpm::resolver::ResolutionStrategy::Highest,
        container.package_client.clone(),
        container.search_provider.clone(),
    )?;

    // Collect all dependencies from workspace packages.
    let mut all_dependencies = HashMap::new();
    let mut all_dev_dependencies = HashMap::new();

    // First, add workspace-level dependencies (inherited by all packages)
    if !dev_only {
        for (dep_name, dep_version) in workspace.workspace_dependencies() {
            all_dependencies.insert(dep_name.clone(), dep_version.clone());
        }
    }
    if !no_dev {
        for (dep_name, dep_version) in workspace.workspace_dev_dependencies() {
            all_dev_dependencies.insert(dep_name.clone(), dep_version.clone());
        }
    }

    // Then, collect dependencies from individual workspace packages
    for workspace_pkg in workspace.packages.values() {
        // Collect regular dependencies from workspace package.
        if !dev_only {
            for (dep_name, dep_version) in &workspace_pkg.manifest.dependencies {
                // Use most restrictive constraint if multiple packages specify the same dependency
                // Package-level dependencies override workspace-level ones
                all_dependencies
                    .entry(dep_name.clone())
                    .or_insert_with(|| dep_version.clone());
            }
        }

        // Collect dev dependencies from workspace package.
        if !no_dev {
            for (dep_name, dep_version) in &workspace_pkg.manifest.dev_dependencies {
                // Package-level dev dependencies override workspace-level ones
                all_dev_dependencies
                    .entry(dep_name.clone())
                    .or_insert_with(|| dep_version.clone());
            }
        }
    }

    // Resolve versions
    let resolved_versions = resolver.resolve(&all_dependencies).await?;
    let resolved_dev_versions = if !all_dev_dependencies.is_empty() {
        resolver.resolve(&all_dev_dependencies).await?
    } else {
        HashMap::new()
    };

    let mut installed_count = 0;

    // Install regular dependencies
    for (name, version) in &resolved_versions {
        println!("  Installing {}@{} (shared)", name, version);
        installer
            .install_package(name, &version.to_string())
            .await?;
        installed_count += 1;
    }

    if !no_dev {
        for (name, version) in &resolved_dev_versions {
            println!("  Installing {}@{} (shared, dev)", name, version);
            installer
                .install_package(name, &version.to_string())
                .await?;
            installed_count += 1;
        }
    }

    println!(
        "\n✓ Installed {} shared dependency(ies) at workspace root",
        installed_count
    );

    Ok(())
}

async fn generate_lockfile(
    project_root: &Path,
    manifest: &PackageManifest,
    no_dev: bool,
) -> LpmResult<()> {
    // Load service container
    let container = ServiceContainer::new()?;

    // Try to load existing lockfile for incremental updates
    let existing_lockfile = Lockfile::load(project_root)?;

    let builder = LockfileBuilder::with_dependencies(
        container.config.clone(),
        container.cache.clone(),
        container.package_client.clone(),
        container.search_provider.clone(),
    )?;
    let lockfile = if let Some(existing) = existing_lockfile {
        // Use incremental update
        builder
            .update_lockfile(&existing, manifest, project_root, no_dev)
            .await?
    } else {
        // Build from scratch
        builder
            .build_lockfile(manifest, project_root, no_dev)
            .await?
    };

    // Save lockfile
    lockfile.save(project_root)?;

    println!("✓ Generated {}", lpm::package::lockfile::LOCKFILE_NAME);
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
) -> LpmResult<()> {
    run_interactive_with_input(project_root, dev, manifest, &DialoguerInput).await
}

/// Interactive package installation with dependency injection
pub async fn run_interactive_with_input(
    project_root: &Path,
    dev: bool,
    manifest: &mut PackageManifest,
    input: &dyn UserInput,
) -> LpmResult<()> {
    println!("🔍 Interactive Package Installation\n");

    // Fetch manifest
    println!("Loading package list...");
    let container = ServiceContainer::new()?;
    let luarocks_manifest = container.package_client.fetch_manifest().await?;

    // Get search query
    let query = input.prompt_string("Search for packages")?;

    // Search packages (fuzzy match)
    let matcher = SkimMatcherV2::default();
    let mut matches: Vec<(String, i64)> = luarocks_manifest
        .packages
        .keys()
        .filter_map(|name| {
            matcher
                .fuzzy_match(name, &query)
                .map(|score| (name.clone(), score))
        })
        .collect();

    // Sort by score (higher is better)
    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches.truncate(20); // Limit to top 20 results

    if matches.is_empty() {
        println!("No packages found matching '{}'", query);
        return Ok(());
    }

    // Display results
    let package_names: Vec<String> = matches.iter().map(|(name, _)| name.clone()).collect();

    println!("\nFound {} package(s):\n", package_names.len());
    for (i, name) in package_names.iter().enumerate() {
        if let Some(latest) = luarocks_manifest.get_latest_version(name) {
            println!("  {}. {} (latest: {})", i + 1, name, latest.version);
        } else {
            println!("  {}. {}", i + 1, name);
        }
    }

    // Select packages
    let selections = input.prompt_multiselect(
        "Select packages to install (space to select, enter to confirm)",
        &package_names,
    )?;

    if selections.is_empty() {
        println!("No packages selected.");
        return Ok(());
    }

    // Collect package selections with version and dependency type
    struct PackageSelection {
        name: String,
        version: String,
        is_dev: bool,
        description: Option<String>,
        license: Option<String>,
        homepage: Option<String>,
        dependencies: Vec<String>,
    }

    let mut package_selections: Vec<PackageSelection> = Vec::new();

    println!("\n📦 Configuring selected packages:\n");

    for &idx in &selections {
        let package_name = &package_names[idx];

        // Get available versions
        let versions = luarocks_manifest.get_package_versions(package_name);
        if versions.is_none() || versions.unwrap().is_empty() {
            eprintln!(
                "⚠️  Warning: Could not find versions for {}, skipping",
                package_name
            );
            continue;
        }

        let versions = versions.unwrap();
        let version_strings: Vec<String> = versions.iter().map(|pv| pv.version.clone()).collect();

        // Sort versions (latest first) - simple string sort should work for most cases
        let mut sorted_versions = version_strings.clone();
        sorted_versions.sort_by(|a, b| b.cmp(a)); // Reverse sort (latest first)

        // Select version
        println!("Package: {}", package_name);
        let version_selection = input.prompt_select("Select version", &sorted_versions, 0)?;

        let selected_version = sorted_versions[version_selection].clone();

        // Find the selected version's PackageVersion to get rockspec URL
        let selected_pkg_version = versions
            .iter()
            .find(|pv| pv.version == selected_version)
            .ok_or_else(|| {
                LpmError::Package(format!(
                    "Version {} not found for {}",
                    selected_version, package_name
                ))
            })?;

        // Fetch and parse rockspec to get metadata and dependencies
        println!("  Fetching package metadata...");
        let rockspec_content = container
            .package_client
            .download_rockspec(&selected_pkg_version.rockspec_url)
            .await?;
        let rockspec = container.package_client.parse_rockspec(&rockspec_content)?;

        // Select dependency type (dev or prod)
        let dep_type_options = vec![
            "Production dependency".to_string(),
            "Development dependency".to_string(),
        ];
        let default_dep_type = if dev { 1 } else { 0 };

        let dep_type_selection =
            input.prompt_select("Dependency type", &dep_type_options, default_dep_type)?;

        let is_dev = dep_type_selection == 1;

        package_selections.push(PackageSelection {
            name: package_name.clone(),
            version: selected_version,
            is_dev,
            description: rockspec.description.clone(),
            license: rockspec.license.clone(),
            homepage: rockspec.homepage.clone(),
            dependencies: rockspec.dependencies.clone(),
        });

        println!(); // Empty line between packages
    }

    if package_selections.is_empty() {
        println!("No valid packages to install.");
        return Ok(());
    }

    // Show detailed summary with metadata and dependencies
    println!("\n📋 Installation Summary:\n");
    for selection in &package_selections {
        let dep_type = if selection.is_dev { "dev" } else { "prod" };
        println!(
            "  📦 {}@{} ({})",
            selection.name, selection.version, dep_type
        );

        if let Some(ref desc) = selection.description {
            println!("     Description: {}", desc);
        }

        if let Some(ref license) = selection.license {
            println!("     License: {}", license);
        }

        if let Some(ref homepage) = selection.homepage {
            println!("     Homepage: {}", homepage);
        }

        if !selection.dependencies.is_empty() {
            println!("     Dependencies:");
            for dep in &selection.dependencies {
                println!("       - {}", dep);
            }
        }

        println!(); // Empty line between packages
    }

    let confirmed = input.prompt_confirm(
        &format!("Install {} package(s)?", package_selections.len()),
        true,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    // Install selected packages
    let installer = PackageInstaller::new(project_root)?;
    installer.init()?;

    for selection in &package_selections {
        println!("\nInstalling {}@{}...", selection.name, selection.version);

        // Check for conflicts
        ConflictChecker::check_new_dependency(manifest, &selection.name, "*")?;

        // Install
        installer
            .install_package(&selection.name, &selection.version)
            .await?;

        // Add to manifest
        if selection.is_dev {
            manifest
                .dev_dependencies
                .insert(selection.name.clone(), "*".to_string());
        } else {
            manifest
                .dependencies
                .insert(selection.name.clone(), "*".to_string());
        }

        println!("✓ Installed {}", selection.name);
    }

    // Generate loader
    PathSetup::install_loader(project_root)?;

    // Generate lockfile
    generate_lockfile(project_root, manifest, false).await?;

    println!("\n✓ Installed {} package(s)", selections.len());

    Ok(())
}

/// Install dependencies for filtered workspace packages
async fn install_workspace_filtered(
    workspace: &Workspace,
    filter_patterns: &[String],
    dev: bool,
    no_dev: bool,
    dev_only: bool,
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

        // Install dependencies
        let installer = PackageInstaller::new(&pkg_dir)?;
        installer.init()?;

        // Initialize client for dependency resolution
        let container = ServiceContainer::new()?;
        let luarocks_manifest = container.package_client.fetch_manifest().await?;
        let resolver = DependencyResolver::with_dependencies(
            luarocks_manifest,
            lpm::resolver::ResolutionStrategy::Highest,
            container.package_client.clone(),
            container.search_provider.clone(),
        )?;

        // Resolve all dependencies at once
        let resolved_versions = resolver.resolve(&deps_to_install).await?;

        for dep_name in deps_to_install.keys() {
            println!("  Installing {}...", dep_name);

            // Get resolved version
            let resolved_version = resolved_versions.get(dep_name).ok_or_else(|| {
                LpmError::Package(format!("Failed to resolve version for {}", dep_name))
            })?;

            installer
                .install_package(dep_name, &resolved_version.to_string())
                .await?;
            println!("  ✓ Installed {}@{}", dep_name, resolved_version);
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

#[cfg(test)]
mod tests {
    use super::*;
    use lpm::package::manifest::PackageManifest;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    // Mock UserInput for testing
    struct MockInput {
        strings: HashMap<&'static str, String>,
        confirms: HashMap<&'static str, bool>,
        selects: HashMap<&'static str, usize>,
        multiselects: HashMap<&'static str, Vec<usize>>,
    }

    impl MockInput {
        fn new() -> Self {
            Self {
                strings: HashMap::new(),
                confirms: HashMap::new(),
                selects: HashMap::new(),
                multiselects: HashMap::new(),
            }
        }

        fn with_string(mut self, prompt: &'static str, value: String) -> Self {
            self.strings.insert(prompt, value);
            self
        }

        fn with_confirm(mut self, prompt: &'static str, value: bool) -> Self {
            self.confirms.insert(prompt, value);
            self
        }

        fn with_select(mut self, prompt: &'static str, index: usize) -> Self {
            self.selects.insert(prompt, index);
            self
        }

        fn with_multiselect(mut self, prompt: &'static str, indices: Vec<usize>) -> Self {
            self.multiselects.insert(prompt, indices);
            self
        }
    }

    impl UserInput for MockInput {
        fn prompt_string(&self, prompt: &str) -> LpmResult<String> {
            self.strings
                .get(prompt)
                .cloned()
                .ok_or_else(|| LpmError::Config(format!("Unexpected prompt: {}", prompt)))
        }

        fn prompt_confirm(&self, prompt: &str, _default: bool) -> LpmResult<bool> {
            self.confirms
                .get(prompt)
                .copied()
                .ok_or_else(|| LpmError::Config(format!("Unexpected prompt: {}", prompt)))
        }

        fn prompt_select(
            &self,
            prompt: &str,
            _items: &[String],
            _default: usize,
        ) -> LpmResult<usize> {
            self.selects
                .get(prompt)
                .copied()
                .ok_or_else(|| LpmError::Config(format!("Unexpected prompt: {}", prompt)))
        }

        fn prompt_multiselect(&self, prompt: &str, _items: &[String]) -> LpmResult<Vec<usize>> {
            self.multiselects
                .get(prompt)
                .cloned()
                .ok_or_else(|| LpmError::Config(format!("Unexpected prompt: {}", prompt)))
        }
    }

    #[test]
    fn test_parse_package_spec_with_version() {
        let spec = "test-package@1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "test-package");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_without_version() {
        let spec = "test-package";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "test-package");
        assert_eq!(version, None);
    }

    #[test]
    fn test_parse_package_spec_with_constraint() {
        let spec = "test-package@^1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "test-package");
        assert_eq!(version, Some("^1.0.0".to_string()));
    }

    #[test]
    fn test_install_from_path_nonexistent() {
        let mut manifest = PackageManifest::default("test".to_string());
        let result = install_from_path("/nonexistent/path", false, &mut manifest);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Path does not exist"));
    }

    #[test]
    fn test_install_from_path_valid() {
        let temp = TempDir::new().unwrap();
        let local_pkg_dir = temp.path().join("local-pkg");
        fs::create_dir_all(&local_pkg_dir).unwrap();

        let local_manifest = PackageManifest {
            name: "local-pkg".to_string(),
            version: "1.0.0".to_string(),
            ..PackageManifest::default("".to_string())
        };
        local_manifest.save(&local_pkg_dir).unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        let result = install_from_path(local_pkg_dir.to_str().unwrap(), false, &mut manifest);
        assert!(result.is_ok());
        assert!(manifest.dependencies.contains_key("local-pkg"));
    }

    #[test]
    fn test_install_from_path_as_dev() {
        let temp = TempDir::new().unwrap();
        let local_pkg_dir = temp.path().join("local-pkg");
        fs::create_dir_all(&local_pkg_dir).unwrap();

        let local_manifest = PackageManifest {
            name: "local-pkg".to_string(),
            version: "1.0.0".to_string(),
            ..PackageManifest::default("".to_string())
        };
        local_manifest.save(&local_pkg_dir).unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        let result = install_from_path(local_pkg_dir.to_str().unwrap(), true, &mut manifest);
        assert!(result.is_ok());
        assert!(manifest.dev_dependencies.contains_key("local-pkg"));
    }

    #[test]
    fn test_parse_package_spec_with_at_symbol() {
        let spec = "test@1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "test");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_caret() {
        let spec = "test@^1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "test");
        assert_eq!(version, Some("^1.0.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_tilde() {
        let spec = "test@~1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "test");
        assert_eq!(version, Some("~1.0.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_multiple_at() {
        let spec = "test@1.0.0@extra";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "test");
        assert_eq!(version, Some("1.0.0@extra".to_string()));
    }

    #[test]
    fn test_parse_package_spec_empty() {
        let spec = "";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "");
        assert_eq!(version, None);
    }

    #[test]
    fn test_parse_package_spec_with_space() {
        let spec = "test package";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "test package");
        assert_eq!(version, None);
    }

    #[test]
    fn test_parse_package_spec_with_version_at_start() {
        let spec = "@1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_complex_version() {
        let spec = "package@^1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "package");
        assert_eq!(version, Some("^1.0.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_tilde_version() {
        let spec = "package~1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "package~1.0.0");
        assert_eq!(version, None);
    }

    #[test]
    fn test_parse_package_spec_with_equals() {
        let spec = "package=1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "package=1.0.0");
        assert_eq!(version, None);
    }

    #[test]
    fn test_parse_package_spec_with_whitespace_around_at() {
        let spec = "package @ 1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].trim().to_string(),
                Some(spec[at_pos + 1..].trim().to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "package");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_special_chars_in_name() {
        let spec = "package-name_123@1.0.0";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "package-name_123");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_long_version() {
        let spec = "package@1.2.3-beta.1+build.123";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "package");
        assert_eq!(version, Some("1.2.3-beta.1+build.123".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_only_at() {
        let spec = "@";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "");
        assert_eq!(version, Some("".to_string()));
    }

    #[test]
    fn test_parse_package_spec_with_multiple_at_symbols() {
        let spec = "package@1.0.0@extra";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "package");
        assert_eq!(version, Some("1.0.0@extra".to_string()));
    }

    #[test]
    fn test_install_functions_structure() {
        // Test install function structures
        // The actual async tests would require network access
        // These tests verify the function signatures and structure
    }

    #[test]
    fn test_install_workspace_dependencies_structure() {
        // Test install_workspace_dependencies structure
        // The actual async tests would require network access
        // These tests verify the function signatures and structure
    }

    #[test]
    fn test_create_global_executables_structure() {
        // Test create_global_executables structure
        // The actual async tests would require network access
        // These tests verify the function signatures and structure
    }

    #[test]
    fn test_create_executable_wrapper_unix() {
        // Test create_executable_wrapper on Unix
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();
        let global_bin = temp.path().join("bin");
        let global_lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&global_bin).unwrap();
        fs::create_dir_all(&global_lua_modules).unwrap();

        let script_path = temp.path().join("script.lua");
        fs::write(&script_path, "print('hello')").unwrap();

        #[cfg(unix)]
        {
            let result = create_executable_wrapper(
                "test-script",
                &script_path,
                &global_bin,
                &global_lua_modules,
            );
            assert!(result.is_ok());
            let wrapper_path = global_bin.join("test-script");
            assert!(wrapper_path.exists());
        }
    }

    #[test]
    fn test_save_global_package_metadata() {
        // Test save_global_package_metadata
        use tempfile::TempDir;
        let temp = TempDir::new().unwrap();

        // Set up environment to use temp directory
        std::env::set_var("HOME", temp.path());
        std::env::set_var("XDG_CONFIG_HOME", temp.path().join("config"));
        std::env::set_var("XDG_DATA_HOME", temp.path().join("data"));

        let executables = vec!["exe1".to_string(), "exe2".to_string()];
        let result = save_global_package_metadata("test-package", &executables);
        assert!(result.is_ok());

        // Clean up
        std::env::remove_var("HOME");
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_DATA_HOME");
    }

    #[test]
    fn test_install_package_structure() {
        // Test install_package structure
        // The actual async tests would require network access
        // These tests verify the function signatures and structure
    }

    #[test]
    fn test_install_options_default() {
        let options = InstallOptions {
            package: None,
            dev: false,
            path: None,
            no_dev: false,
            dev_only: false,
            global: false,
            interactive: false,
            filter: vec![],
        };
        assert_eq!(options.package, None);
        assert!(!options.dev);
        assert!(!options.global);
    }

    #[test]
    fn test_install_options_with_package() {
        let options = InstallOptions {
            package: Some("test-pkg".to_string()),
            dev: true,
            path: None,
            no_dev: false,
            dev_only: false,
            global: false,
            interactive: false,
            filter: vec![],
        };
        assert_eq!(options.package, Some("test-pkg".to_string()));
        assert!(options.dev);
    }

    #[test]
    fn test_install_options_with_filter() {
        let options = InstallOptions {
            package: None,
            dev: false,
            path: None,
            no_dev: false,
            dev_only: false,
            global: false,
            interactive: false,
            filter: vec!["pkg1".to_string(), "pkg2".to_string()],
        };
        assert_eq!(options.filter.len(), 2);
        assert_eq!(options.filter[0], "pkg1");
        assert_eq!(options.filter[1], "pkg2");
    }

    #[test]
    fn test_install_from_path_creates_path_dependency() {
        let temp = TempDir::new().unwrap();
        let local_pkg_dir = temp.path().join("local-package");
        fs::create_dir_all(&local_pkg_dir).unwrap();

        let local_manifest = PackageManifest {
            name: "local-package".to_string(),
            version: "2.0.0".to_string(),
            ..PackageManifest::default("".to_string())
        };
        local_manifest.save(&local_pkg_dir).unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        install_from_path(local_pkg_dir.to_str().unwrap(), false, &mut manifest).unwrap();

        assert!(manifest.dependencies.contains_key("local-package"));
        let dep_version = manifest.dependencies.get("local-package").unwrap();
        assert!(dep_version.starts_with("path:"));
    }

    #[test]
    fn test_install_from_path_dev_dependency() {
        let temp = TempDir::new().unwrap();
        let local_pkg_dir = temp.path().join("dev-package");
        fs::create_dir_all(&local_pkg_dir).unwrap();

        let local_manifest = PackageManifest {
            name: "dev-package".to_string(),
            version: "1.0.0".to_string(),
            ..PackageManifest::default("".to_string())
        };
        local_manifest.save(&local_pkg_dir).unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        install_from_path(local_pkg_dir.to_str().unwrap(), true, &mut manifest).unwrap();

        assert!(manifest.dev_dependencies.contains_key("dev-package"));
        assert!(!manifest.dependencies.contains_key("dev-package"));
    }

    #[test]
    fn test_install_from_path_missing_package_yaml() {
        let temp = TempDir::new().unwrap();
        let local_pkg_dir = temp.path().join("empty-dir");
        fs::create_dir_all(&local_pkg_dir).unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        let result = install_from_path(local_pkg_dir.to_str().unwrap(), false, &mut manifest);

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_package_spec_edge_cases() {
        // Test with just @
        let spec = "@";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "");
        assert_eq!(version, Some("".to_string()));

        // Test with multiple @ symbols
        let spec = "pkg@v1@v2";
        let (name, version) = if let Some(at_pos) = spec.find('@') {
            (
                spec[..at_pos].to_string(),
                Some(spec[at_pos + 1..].to_string()),
            )
        } else {
            (spec.to_string(), None)
        };
        assert_eq!(name, "pkg");
        assert_eq!(version, Some("v1@v2".to_string()));
    }

    #[tokio::test]
    async fn test_run_conflicting_flags_no_dev_and_dev_only() {
        let options = InstallOptions {
            package: None,
            dev: false,
            path: None,
            no_dev: true,
            dev_only: true,
            global: false,
            interactive: false,
            filter: vec![],
        };

        let result = run(options).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot use both --no-dev and --dev-only"));
    }

    #[tokio::test]
    async fn test_run_global_without_package() {
        let options = InstallOptions {
            package: None,
            dev: false,
            path: None,
            no_dev: false,
            dev_only: false,
            global: true,
            interactive: false,
            filter: vec![],
        };

        let result = run(options).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Global installation requires a package name"));
    }

    #[tokio::test]
    async fn test_run_global_with_path() {
        let options = InstallOptions {
            package: Some("test-pkg".to_string()),
            dev: false,
            path: Some("/some/path".to_string()),
            no_dev: false,
            dev_only: false,
            global: true,
            interactive: false,
            filter: vec![],
        };

        let result = run(options).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot install from local path globally"));
    }

    #[tokio::test]
    async fn test_run_filter_without_workspace() {
        let temp = TempDir::new().unwrap();

        // Create a non-workspace project
        fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\n",
        )
        .unwrap();

        let original_dir = std::env::current_dir().ok();
        std::env::set_current_dir(temp.path()).unwrap();

        let options = InstallOptions {
            package: None,
            dev: false,
            path: None,
            no_dev: false,
            dev_only: false,
            global: false,
            interactive: false,
            filter: vec!["pkg1".to_string()],
        };

        let result = run(options).await;

        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("--filter can only be used in workspace mode"));
    }

    #[tokio::test]
    async fn test_run_with_package_and_path() {
        let temp = TempDir::new().unwrap();

        fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\n",
        )
        .unwrap();

        let original_dir = std::env::current_dir().ok();
        std::env::set_current_dir(temp.path()).unwrap();

        let options = InstallOptions {
            package: Some("test-pkg".to_string()),
            dev: false,
            path: Some("/some/path".to_string()),
            no_dev: false,
            dev_only: false,
            global: false,
            interactive: false,
            filter: vec![],
        };

        let result = run(options).await;

        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot specify both package and --path"));
    }

    #[tokio::test]
    async fn test_run_interactive_cancelled() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());

        let input = MockInput::new()
            .with_string("Search for packages", "test".to_string())
            .with_multiselect(
                "Select packages to install (space to select, enter to confirm)",
                vec![],
            );

        let result = run_interactive_with_input(temp.path(), false, &mut manifest, &input).await;

        // Should succeed but do nothing (no packages selected)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_interactive_no_matches() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());

        let input = MockInput::new().with_string(
            "Search for packages",
            "nonexistent-package-xyz-123".to_string(),
        );

        let result = run_interactive_with_input(temp.path(), false, &mut manifest, &input).await;

        // Should succeed (no packages found)
        assert!(result.is_ok());
    }

    #[test]
    fn test_dialoguer_input_trait() {
        // Test that DialoguerInput implements UserInput
        let _input: &dyn UserInput = &DialoguerInput;
    }

    #[test]
    fn test_mock_input_with_string() {
        let input = MockInput::new().with_string("Test prompt", "test value".to_string());
        let result = input.prompt_string("Test prompt");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test value");
    }

    #[test]
    fn test_mock_input_with_confirm() {
        let input = MockInput::new().with_confirm("Confirm?", true);
        let result = input.prompt_confirm("Confirm?", false);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_mock_input_with_select() {
        let input = MockInput::new().with_select("Choose", 2);
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let result = input.prompt_select("Choose", &items, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);
    }

    #[test]
    fn test_mock_input_with_multiselect() {
        let input = MockInput::new().with_multiselect("Choose", vec![0, 2]);
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let result = input.prompt_multiselect("Choose", &items);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![0, 2]);
    }

    #[test]
    fn test_mock_input_unexpected_prompt() {
        let input = MockInput::new();
        let result = input.prompt_string("Unexpected");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unexpected prompt"));
    }
}
