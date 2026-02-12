use depot::core::path::{find_project_root, lua_modules_dir};
use depot::core::{DepotError, DepotResult};
use std::env;
use std::fs;

pub fn run() -> DepotResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| DepotError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;
    let lua_modules = lua_modules_dir(&project_root);

    if !lua_modules.exists() {
        println!("lua_modules directory does not exist. Nothing to clean.");
        return Ok(());
    }

    println!("Cleaning lua_modules directory...");

    // Count packages before cleaning
    let package_count = count_packages(&lua_modules)?;

    // Remove lua_modules directory
    fs::remove_dir_all(&lua_modules)?;

    println!("✓ Cleaned {} package(s)", package_count);
    println!("  Removed: {}", lua_modules.display());

    Ok(())
}

fn count_packages(lua_modules: &std::path::Path) -> DepotResult<usize> {
    let mut count = 0;

    if lua_modules.exists() {
        for entry in fs::read_dir(lua_modules)? {
            let entry = entry?;
            let path = entry.path();

            // Skip .depot metadata directory
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == ".depot")
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                count += 1;
            }
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_count_packages() {
        let temp = TempDir::new().unwrap();
        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        // Create some package directories
        fs::create_dir_all(lua_modules.join("package1")).unwrap();
        fs::create_dir_all(lua_modules.join("package2")).unwrap();
        fs::create_dir_all(lua_modules.join("package3")).unwrap();

        let count = count_packages(&lua_modules).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_packages_skips_lpm_metadata() {
        let temp = TempDir::new().unwrap();
        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        // Create package directories
        fs::create_dir_all(lua_modules.join("package1")).unwrap();
        // Create .depot metadata directory (should be skipped)
        fs::create_dir_all(lua_modules.join(".depot")).unwrap();

        let count = count_packages(&lua_modules).unwrap();
        assert_eq!(count, 1); // .depot should be skipped
    }

    #[test]
    fn test_count_packages_empty() {
        let temp = TempDir::new().unwrap();
        let lua_modules = temp.path().join("lua_modules");
        fs::create_dir_all(&lua_modules).unwrap();

        let count = count_packages(&lua_modules).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_packages_nonexistent() {
        let temp = TempDir::new().unwrap();
        let lua_modules = temp.path().join("nonexistent");

        let count = count_packages(&lua_modules).unwrap();
        assert_eq!(count, 0);
    }
}
