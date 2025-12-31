use lpm::core::path::find_project_root;
use lpm::core::{LpmError, LpmResult};
use lpm::package::installer::PackageInstaller;
use lpm::package::manifest::PackageManifest;
use std::env;

pub fn run(package: String, global: bool) -> LpmResult<()> {
    if global {
        return remove_global(&package);
    }

    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;
    let mut manifest = PackageManifest::load(&project_root)?;

    // Try to remove from dependencies
    let removed_from_deps = manifest.dependencies.remove(&package).is_some();

    // Try to remove from dev_dependencies
    let removed_from_dev = manifest.dev_dependencies.remove(&package).is_some();

    if !removed_from_deps && !removed_from_dev {
        return Err(LpmError::Package(format!(
            "Package '{}' not found in dependencies or dev_dependencies",
            package
        )));
    }

    // Actually remove package files from lua_modules/
    let installer = PackageInstaller::new(&project_root)?;
    if installer.is_installed(&package) {
        installer.remove_package(&package)?;
    }

    // Save updated manifest
    manifest.save(&project_root)?;

    let location = if removed_from_deps && removed_from_dev {
        "dependencies and dev_dependencies"
    } else if removed_from_deps {
        "dependencies"
    } else {
        "dev_dependencies"
    };

    println!("✓ Removed {} from {}", package, location);
    println!("✓ Removed package files from lua_modules/");

    Ok(())
}

fn remove_global(package: &str) -> LpmResult<()> {
    use lpm::core::path::{global_bin_dir, global_lua_modules_dir, global_packages_metadata_dir};
    use serde::Deserialize;
    use std::fs;

    let global_lua_modules = global_lua_modules_dir()?;
    let global_bin = global_bin_dir()?;
    let metadata_dir = global_packages_metadata_dir()?;

    // Check if package is installed globally
    let package_dir = global_lua_modules.join(package);
    if !package_dir.exists() {
        return Err(LpmError::Package(format!(
            "Package '{}' is not installed globally",
            package
        )));
    }

    // Load metadata to find executables
    let metadata_file = metadata_dir.join(format!("{}.yaml", package));
    let executables = if metadata_file.exists() {
        #[derive(Deserialize)]
        struct GlobalPackageMetadata {
            executables: Vec<String>,
        }

        if let Ok(content) = fs::read_to_string(&metadata_file) {
            if let Ok(metadata) = serde_yaml::from_str::<GlobalPackageMetadata>(&content) {
                metadata.executables
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Remove executables from global bin
    for exe_name in &executables {
        let exe_path = global_bin.join(exe_name);
        #[cfg(windows)]
        let exe_path = global_bin.join(format!("{}.bat", exe_name));

        if exe_path.exists() {
            fs::remove_file(&exe_path)?;
            println!("  ✓ Removed global executable: {}", exe_name);
        }
    }

    // Remove package directory
    fs::remove_dir_all(&package_dir)?;

    // Remove metadata file
    if metadata_file.exists() {
        fs::remove_file(&metadata_file)?;
    }

    println!("✓ Uninstalled {} globally", package);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpm::package::manifest::PackageManifest;

    #[test]
    fn test_remove_global_function_exists() {
        // Test that remove_global function exists
        let _ = remove_global;
    }

    #[test]
    fn test_remove_location_determination() {
        // Test the logic for determining removal location
        let mut manifest = PackageManifest::default("test".to_string());

        // Test removing from dependencies only
        manifest
            .dependencies
            .insert("pkg1".to_string(), "1.0.0".to_string());
        let removed_from_deps = manifest.dependencies.remove("pkg1").is_some();
        let removed_from_dev = false;

        let location = if removed_from_deps && removed_from_dev {
            "dependencies and dev_dependencies"
        } else if removed_from_deps {
            "dependencies"
        } else {
            "dev_dependencies"
        };
        assert_eq!(location, "dependencies");

        // Test removing from dev_dependencies only
        manifest
            .dev_dependencies
            .insert("pkg2".to_string(), "1.0.0".to_string());
        let removed_from_deps = false;
        let removed_from_dev = manifest.dev_dependencies.remove("pkg2").is_some();

        let location = if removed_from_deps && removed_from_dev {
            "dependencies and dev_dependencies"
        } else if removed_from_deps {
            "dependencies"
        } else {
            "dev_dependencies"
        };
        assert_eq!(location, "dev_dependencies");
    }

    #[test]
    fn test_remove_location_both() {
        // Test removal from both deps and dev_deps
        let mut manifest = PackageManifest::default("test".to_string());

        manifest
            .dependencies
            .insert("pkg".to_string(), "1.0.0".to_string());
        manifest
            .dev_dependencies
            .insert("pkg".to_string(), "1.0.0".to_string());

        let removed_from_deps = manifest.dependencies.remove("pkg").is_some();
        let removed_from_dev = manifest.dev_dependencies.remove("pkg").is_some();

        let location = if removed_from_deps && removed_from_dev {
            "dependencies and dev_dependencies"
        } else if removed_from_deps {
            "dependencies"
        } else {
            "dev_dependencies"
        };
        assert_eq!(location, "dependencies and dev_dependencies");
    }

    #[test]
    fn test_remove_global_not_installed() {
        // Test removing a package that's not installed globally
        let result = remove_global("nonexistent-global-package-12345");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not installed globally"));
    }

    #[test]
    fn test_manifest_dependency_removal() {
        let mut manifest = PackageManifest::default("test".to_string());

        // Add multiple dependencies
        manifest
            .dependencies
            .insert("dep1".to_string(), "^1.0.0".to_string());
        manifest
            .dependencies
            .insert("dep2".to_string(), "^2.0.0".to_string());

        assert_eq!(manifest.dependencies.len(), 2);

        // Remove one
        manifest.dependencies.remove("dep1");
        assert_eq!(manifest.dependencies.len(), 1);
        assert!(manifest.dependencies.contains_key("dep2"));
    }

    #[test]
    fn test_manifest_dev_dependency_removal() {
        let mut manifest = PackageManifest::default("test".to_string());

        // Add dev dependencies
        manifest
            .dev_dependencies
            .insert("dev1".to_string(), "^1.0.0".to_string());
        manifest
            .dev_dependencies
            .insert("dev2".to_string(), "^2.0.0".to_string());

        assert_eq!(manifest.dev_dependencies.len(), 2);

        // Remove one
        manifest.dev_dependencies.remove("dev1");
        assert_eq!(manifest.dev_dependencies.len(), 1);
        assert!(manifest.dev_dependencies.contains_key("dev2"));
    }

    #[test]
    fn test_run_function_exists() {
        let _ = run;
    }
}
