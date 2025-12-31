use lpm::core::path::{find_project_root, lua_modules_dir};
use lpm::core::{LpmError, LpmResult};
use lpm::package::lockfile::Lockfile;
use lpm::package::manifest::PackageManifest;
use std::env;

pub fn run(tree: bool, global: bool) -> LpmResult<()> {
    if global {
        return list_global();
    }

    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;

    // Load manifest
    let manifest = PackageManifest::load(&project_root)?;

    // Load lockfile if it exists
    let lockfile = Lockfile::load(&project_root)?;

    let lua_modules = lua_modules_dir(&project_root);

    if tree {
        // Show dependency tree
        print_dependency_tree(&manifest, &lockfile, &lua_modules, "", true)?;
    } else {
        // Show flat list
        print_package_list(&manifest, &lockfile, &lua_modules)?;
    }

    Ok(())
}

fn list_global() -> LpmResult<()> {
    use lpm::core::path::{global_lua_modules_dir, global_packages_metadata_dir};
    use serde::Deserialize;
    use std::fs;

    let global_lua_modules = global_lua_modules_dir()?;

    if !global_lua_modules.exists() {
        println!("No globally installed packages.");
        return Ok(());
    }

    let mut packages = Vec::new();

    if let Ok(entries) = fs::read_dir(&global_lua_modules) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip .lpm metadata directory
                    if name != ".lpm" {
                        packages.push(name.to_string());
                    }
                }
            }
        }
    }

    if packages.is_empty() {
        println!("No globally installed packages.");
        return Ok(());
    }

    packages.sort();

    // Load metadata to show executables
    let metadata_dir = global_packages_metadata_dir().ok();

    println!("Globally installed packages:");
    for package in packages {
        // Try to load metadata to show executables
        let mut executables = Vec::new();
        if let Some(ref meta_dir) = metadata_dir {
            let metadata_file = meta_dir.join(format!("{}.yaml", package));
            if metadata_file.exists() {
                #[derive(Deserialize)]
                struct GlobalPackageMetadata {
                    executables: Vec<String>,
                }

                if let Ok(content) = fs::read_to_string(&metadata_file) {
                    if let Ok(metadata) = serde_yaml::from_str::<GlobalPackageMetadata>(&content) {
                        executables = metadata.executables;
                    }
                }
            }
        }

        if executables.is_empty() {
            println!("  {}", package);
        } else {
            println!("  {} (executables: {})", package, executables.join(", "));
        }
    }

    Ok(())
}

fn print_package_list(
    manifest: &PackageManifest,
    lockfile: &Option<Lockfile>,
    lua_modules: &std::path::Path,
) -> LpmResult<()> {
    println!("Dependencies:");

    if manifest.dependencies.is_empty() && manifest.dev_dependencies.is_empty() {
        println!("  (none)");
        return Ok(());
    }

    // Print regular dependencies
    for (name, version_constraint) in &manifest.dependencies {
        let installed = lua_modules.join(name).exists();
        let status = if installed { "✓" } else { "✗" };

        // Get resolved version from lockfile if available
        let resolved_version = lockfile
            .as_ref()
            .and_then(|lf| lf.get_package(name))
            .map(|pkg| pkg.version.clone());

        if let Some(version) = resolved_version {
            println!(
                "  {} {}@{} (constraint: {})",
                status, name, version, version_constraint
            );
        } else {
            println!("  {} {} (constraint: {})", status, name, version_constraint);
        }
    }

    // Print dev dependencies
    if !manifest.dev_dependencies.is_empty() {
        println!("\nDev Dependencies:");
        for (name, version_constraint) in &manifest.dev_dependencies {
            let installed = lua_modules.join(name).exists();
            let status = if installed { "✓" } else { "✗" };

            let resolved_version = lockfile
                .as_ref()
                .and_then(|lf| lf.get_package(name))
                .map(|pkg| pkg.version.clone());

            if let Some(version) = resolved_version {
                println!(
                    "  {} {}@{} (constraint: {}, dev)",
                    status, name, version, version_constraint
                );
            } else {
                println!(
                    "  {} {} (constraint: {}, dev)",
                    status, name, version_constraint
                );
            }
        }
    }

    Ok(())
}

fn print_dependency_tree(
    manifest: &PackageManifest,
    lockfile: &Option<Lockfile>,
    lua_modules: &std::path::Path,
    prefix: &str,
    _is_last: bool,
) -> LpmResult<()> {
    // Collect all dependencies
    let all_deps: Vec<(&String, &String, bool)> = manifest
        .dependencies
        .iter()
        .map(|(n, v)| (n, v, false))
        .chain(manifest.dev_dependencies.iter().map(|(n, v)| (n, v, true)))
        .collect();

    for (i, (name, version_constraint, is_dev)) in all_deps.iter().enumerate() {
        let is_last_item = i == all_deps.len() - 1;
        let connector = if is_last_item {
            "└──"
        } else {
            "├──"
        };
        let next_prefix = if is_last_item {
            format!("{}   ", prefix)
        } else {
            format!("{}│  ", prefix)
        };

        let installed = lua_modules.join(*name).exists();
        let status = if installed { "✓" } else { "✗" };

        let resolved_version = lockfile
            .as_ref()
            .and_then(|lf| lf.get_package(name))
            .map(|pkg| pkg.version.clone());

        let dev_label = if *is_dev { " (dev)" } else { "" };

        if let Some(version) = resolved_version {
            println!(
                "{}{} {} {}@{} (constraint: {}){}",
                prefix, connector, status, name, version, version_constraint, dev_label
            );
        } else {
            println!(
                "{}{} {} {} (constraint: {}){}",
                prefix, connector, status, name, version_constraint, dev_label
            );
        }

        // Recursively print dependencies of this package
        if let Some(lockfile) = lockfile {
            if let Some(pkg) = lockfile.get_package(name) {
                if !pkg.dependencies.is_empty() {
                    let deps: Vec<(&String, &String)> = pkg.dependencies.iter().collect();
                    for (j, (dep_name, dep_version)) in deps.iter().enumerate() {
                        let is_last_dep = j == deps.len() - 1;
                        let dep_connector = if is_last_dep {
                            "└──"
                        } else {
                            "├──"
                        };

                        let dep_installed = lua_modules.join(*dep_name).exists();
                        let dep_status = if dep_installed { "✓" } else { "✗" };

                        println!(
                            "{}{} {} {}@{}",
                            next_prefix, dep_connector, dep_status, dep_name, dep_version
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpm::package::lockfile::{LockedPackage, Lockfile};
    use lpm::package::manifest::PackageManifest;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_list_global_function_exists() {
        // Test that list_global function exists and can be called
        let _ = list_global;
    }

    #[test]
    fn test_print_package_list_with_empty_deps() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test".to_string());
        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        let result = print_package_list(&manifest, &None, &lua_modules);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_package_list_with_dependencies() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("test-pkg".to_string(), "1.0.0".to_string());
        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        let result = print_package_list(&manifest, &None, &lua_modules);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_dependency_tree() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("test-pkg".to_string(), "1.0.0".to_string());
        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        let result = print_dependency_tree(&manifest, &None, &lua_modules, "", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_package_list_with_installed_package() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("test-pkg".to_string(), ">=1.0.0".to_string());

        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        // Create package directory to simulate installed package
        fs::create_dir_all(lua_modules.join("test-pkg")).unwrap();

        let result = print_package_list(&manifest, &None, &lua_modules);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_package_list_with_lockfile() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("test-pkg".to_string(), ">=1.0.0".to_string());

        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        // Create lockfile
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

        let result = print_package_list(&manifest, &Some(lockfile), &lua_modules);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_package_list_with_dev_dependencies() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dev_dependencies
            .insert("dev-pkg".to_string(), ">=1.0.0".to_string());

        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        let result = print_package_list(&manifest, &None, &lua_modules);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_dependency_tree_with_lockfile() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("parent-pkg".to_string(), ">=1.0.0".to_string());

        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        // Create lockfile with nested dependencies
        let mut lockfile = Lockfile::new();
        let mut deps = HashMap::new();
        deps.insert("child-pkg".to_string(), "2.0.0".to_string());
        lockfile.add_package(
            "parent-pkg".to_string(),
            LockedPackage {
                version: "1.0.0".to_string(),
                source: "luarocks".to_string(),
                rockspec_url: None,
                source_url: None,
                checksum: "abc".to_string(),
                size: None,
                dependencies: deps,
                build: None,
            },
        );

        let result = print_dependency_tree(&manifest, &Some(lockfile), &lua_modules, "", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_dependency_tree_with_dev_deps() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("prod-pkg".to_string(), ">=1.0.0".to_string());
        manifest
            .dev_dependencies
            .insert("dev-pkg".to_string(), ">=1.0.0".to_string());

        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        let result = print_dependency_tree(&manifest, &None, &lua_modules, "", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_dependency_tree_installed_packages() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("test-pkg".to_string(), ">=1.0.0".to_string());

        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        // Mark as installed
        fs::create_dir_all(lua_modules.join("test-pkg")).unwrap();

        let result = print_dependency_tree(&manifest, &None, &lua_modules, "", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_package_list_both_installed_and_not() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("installed-pkg".to_string(), ">=1.0.0".to_string());
        manifest
            .dependencies
            .insert("not-installed-pkg".to_string(), ">=1.0.0".to_string());

        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();
        fs::create_dir_all(lua_modules.join("installed-pkg")).unwrap();

        let result = print_package_list(&manifest, &None, &lua_modules);
        assert!(result.is_ok());
    }
}
