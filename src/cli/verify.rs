use lpm::cache::Cache;
use lpm::config::Config;
use lpm::core::path::find_project_root;
use lpm::core::{LpmError, LpmResult};
use lpm::package::lockfile::Lockfile;
use lpm::package::verifier::PackageVerifier;
use std::env;

pub fn run() -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;

    // Load lockfile
    let lockfile = Lockfile::load(&project_root)?.ok_or_else(|| {
        LpmError::Package(
            "No package.lock found. Run 'lpm install' first to generate a lockfile.".to_string(),
        )
    })?;

    if lockfile.packages.is_empty() {
        println!("No packages to verify");
        return Ok(());
    }

    // Load config and create cache
    let config = Config::load()?;
    let cache = Cache::new(config.get_cache_dir()?)?;

    // Create verifier
    let verifier = PackageVerifier::new(cache);

    println!("Verifying {} package(s)...", lockfile.packages.len());

    // Verify all packages
    let result = verifier.verify_all(&lockfile, &project_root)?;

    // Display results
    if result.is_success() {
        println!("✓ All packages verified successfully");
        println!("  {} package(s) verified", result.successful.len());
    } else {
        println!("❌ Verification failed");
        println!("  {} package(s) verified", result.successful.len());
        println!("  {} package(s) failed", result.failed.len());

        for (package, error) in &result.failed {
            println!("  ❌ {}: {}", package, error);
        }

        return Err(LpmError::Package(format!(
            "Verification failed for {} package(s)",
            result.failed.len()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use lpm::package::lockfile::{LockedPackage, Lockfile};
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_verify_with_empty_lockfile() {
        // Test that verify handles empty lockfile gracefully
        let _temp = TempDir::new().unwrap();
        let lockfile = Lockfile::new();
        // Should handle empty packages
        assert!(lockfile.packages.is_empty());
    }

    #[test]
    fn test_verify_lockfile_structure() {
        // Test lockfile structure for verification
        let mut lockfile = Lockfile::new();
        let package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None,
            checksum: "abc123".to_string(),
            size: None,
            dependencies: HashMap::new(),
            build: None,
        };
        lockfile.add_package("test-package".to_string(), package);
        assert!(!lockfile.packages.is_empty());
        assert!(lockfile.has_package("test-package"));
    }
}
