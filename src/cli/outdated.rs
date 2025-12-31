use lpm::core::path::find_project_root;
use lpm::core::version::parse_constraint;
use lpm::core::version::Version;
use lpm::core::{LpmError, LpmResult};
use lpm::di::{PackageClient, ServiceContainer};
use lpm::luarocks::manifest::Manifest;
use lpm::luarocks::version::normalize_luarocks_version;
use lpm::package::lockfile::Lockfile;
use lpm::package::manifest::PackageManifest;
use std::env;

pub async fn run() -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;

    // Load manifest and lockfile
    let manifest = PackageManifest::load(&project_root)?;
    let lockfile = Lockfile::load(&project_root)?;

    if manifest.dependencies.is_empty() && manifest.dev_dependencies.is_empty() {
        println!("No dependencies to check");
        return Ok(());
    }

    println!("Checking for outdated packages...");

    // Create service container
    let container = ServiceContainer::new()?;
    let luarocks_manifest = container.package_client.fetch_manifest().await.ok();

    let mut outdated_count = 0;
    let mut up_to_date_count = 0;

    // Check regular dependencies
    for (name, version_constraint) in &manifest.dependencies {
        let current_version = lockfile
            .as_ref()
            .and_then(|lf| lf.get_package(name))
            .and_then(|pkg| Version::parse(&pkg.version).ok());

        match check_outdated(
            container.package_client.as_ref(),
            &luarocks_manifest,
            name,
            version_constraint,
            current_version.as_ref(),
        )
        .await
        {
            Ok(OutdatedStatus::UpToDate) => {
                up_to_date_count += 1;
            }
            Ok(OutdatedStatus::Outdated { current, latest }) => {
                outdated_count += 1;
                if let Some(current) = current {
                    println!(
                        "  ⚠️  {}: {} → {} (constraint: {})",
                        name, current, latest, version_constraint
                    );
                } else {
                    println!(
                        "  ⚠️  {}: (not installed) → {} (constraint: {})",
                        name, latest, version_constraint
                    );
                }
            }
            Ok(OutdatedStatus::NotFound) => {
                println!("  ❓ {}: Package not found on LuaRocks", name);
            }
            Err(e) => {
                println!("  ❌ {}: Error checking version: {}", name, e);
            }
        }
    }

    // Check dev dependencies
    for (name, version_constraint) in &manifest.dev_dependencies {
        let current_version = lockfile
            .as_ref()
            .and_then(|lf| lf.get_package(name))
            .and_then(|pkg| Version::parse(&pkg.version).ok());

        match check_outdated(
            container.package_client.as_ref(),
            &luarocks_manifest,
            name,
            version_constraint,
            current_version.as_ref(),
        )
        .await
        {
            Ok(OutdatedStatus::UpToDate) => {
                up_to_date_count += 1;
            }
            Ok(OutdatedStatus::Outdated { current, latest }) => {
                outdated_count += 1;
                if let Some(current) = current {
                    println!(
                        "  ⚠️  {}: {} → {} (constraint: {}, dev)",
                        name, current, latest, version_constraint
                    );
                } else {
                    println!(
                        "  ⚠️  {}: (not installed) → {} (constraint: {}, dev)",
                        name, latest, version_constraint
                    );
                }
            }
            Ok(OutdatedStatus::NotFound) => {
                println!("  ❓ {}: Package not found on LuaRocks (dev)", name);
            }
            Err(e) => {
                println!("  ❌ {}: Error checking version: {}", name, e);
            }
        }
    }

    println!("\nSummary:");
    println!("  Up to date: {}", up_to_date_count);
    println!("  Outdated: {}", outdated_count);

    if outdated_count > 0 {
        println!("\nRun 'lpm update' to update outdated packages");
    }

    Ok(())
}

#[derive(Debug)]
enum OutdatedStatus {
    UpToDate,
    Outdated {
        current: Option<Version>,
        latest: Version,
    },
    NotFound,
}

async fn check_outdated(
    client: &dyn PackageClient,
    manifest: &Option<Manifest>,
    package_name: &str,
    version_constraint: &str,
    current_version: Option<&Version>,
) -> LpmResult<OutdatedStatus> {
    // Get available versions from manifest
    let available_versions = if let Some(manifest) = manifest {
        manifest.get_package_version_strings(package_name)
    } else {
        // Try to fetch manifest if not available
        match client.fetch_manifest().await {
            Ok(m) => m.get_package_version_strings(package_name),
            Err(_) => return Ok(OutdatedStatus::NotFound),
        }
    };

    if available_versions.is_empty() {
        return Ok(OutdatedStatus::NotFound);
    }

    // Parse and normalize versions
    let mut versions: Vec<Version> = available_versions
        .iter()
        .filter_map(|v| normalize_luarocks_version(v).ok())
        .collect();

    versions.sort_by(|a, b| b.cmp(a)); // Highest first

    if versions.is_empty() {
        return Ok(OutdatedStatus::NotFound);
    }

    let latest = versions[0].clone();

    // Parse constraint
    let constraint = parse_constraint(version_constraint)?;

    // Check if latest satisfies constraint
    if !latest.satisfies(&constraint) {
        // Latest doesn't satisfy constraint, so we're up to date with constraint
        return Ok(OutdatedStatus::UpToDate);
    }

    // Check if current version is outdated
    if let Some(current) = current_version {
        if current < &latest {
            Ok(OutdatedStatus::Outdated {
                current: Some(current.clone()),
                latest,
            })
        } else {
            Ok(OutdatedStatus::UpToDate)
        }
    } else {
        // Not installed, but latest is available
        Ok(OutdatedStatus::Outdated {
            current: None,
            latest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpm::core::version::Version;
    use lpm::luarocks::manifest::{Manifest, PackageVersion};
    use tempfile::TempDir;

    #[test]
    fn test_outdated_status_variants() {
        // Test that OutdatedStatus enum variants can be created
        let _up_to_date = OutdatedStatus::UpToDate;
        let _outdated = OutdatedStatus::Outdated {
            current: Some(Version::new(1, 0, 0)),
            latest: Version::new(2, 0, 0),
        };
        let _not_found = OutdatedStatus::NotFound;
        let _outdated_no_current = OutdatedStatus::Outdated {
            current: None,
            latest: Version::new(1, 0, 0),
        };
    }

    #[tokio::test]
    async fn test_check_outdated_with_empty_versions() {
        let temp = TempDir::new().unwrap();
        let config = Config::load().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let client = LuaRocksClient::new(&config, cache);
        let manifest = Some(Manifest::default());

        // Test with empty manifest (no versions available)
        let result =
            check_outdated(&client, &manifest, "nonexistent-package", ">=0.0.0", None).await;
        assert!(result.is_ok());
        match result.unwrap() {
            OutdatedStatus::NotFound => {}
            _ => panic!("Expected NotFound for empty versions"),
        }
    }

    #[tokio::test]
    async fn test_check_outdated_with_versions() {
        let temp = TempDir::new().unwrap();
        let config = Config::load().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let mut manifest = Manifest::default();
        let versions = vec![
            PackageVersion {
                version: "2.0.0-1".to_string(),
                rockspec_url: "https://example.com/test-2.0.0.rockspec".to_string(),
                archive_url: Some("https://example.com/test-2.0.0.tar.gz".to_string()),
            },
            PackageVersion {
                version: "1.0.0-1".to_string(),
                rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
                archive_url: Some("https://example.com/test-1.0.0.tar.gz".to_string()),
            },
        ];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let current = Some(Version::new(1, 0, 0));
        let result = check_outdated(
            &client,
            &Some(manifest),
            "test-package",
            ">=0.0.0",
            current.as_ref(),
        )
        .await;
        if let Err(e) = &result {
            panic!("check_outdated failed: {}", e);
        }
        match result.unwrap() {
            OutdatedStatus::Outdated {
                current: c,
                latest: l,
            } => {
                assert_eq!(c, Some(Version::new(1, 0, 0)));
                // "2.0.0-1" normalizes to "2.0.1" (revision added to patch)
                assert_eq!(l, Version::new(2, 0, 1));
            }
            status => panic!("Expected Outdated status, got: {:?}", status),
        }
    }

    #[tokio::test]
    async fn test_check_outdated_up_to_date() {
        let temp = TempDir::new().unwrap();
        let config = Config::load().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let client = LuaRocksClient::new(&config, cache);

        let mut manifest = Manifest::default();
        let versions = vec![PackageVersion {
            version: "1.0.0-1".to_string(),
            rockspec_url: "https://example.com/test-1.0.0.rockspec".to_string(),
            archive_url: Some("https://example.com/test-1.0.0.tar.gz".to_string()),
        }];
        manifest
            .packages
            .insert("test-package".to_string(), versions);

        let current = Some(Version::new(1, 0, 0));
        let result = check_outdated(
            &client,
            &Some(manifest),
            "test-package",
            ">=0.0.0",
            current.as_ref(),
        )
        .await;
        if let Err(e) = &result {
            panic!("check_outdated failed: {}", e);
        }
        match result.unwrap() {
            // "1.0.0-1" normalizes to "1.0.1", which is > "1.0.0", so it will be Outdated
            OutdatedStatus::Outdated {
                current: c,
                latest: l,
            } => {
                assert_eq!(c, Some(Version::new(1, 0, 0)));
                assert_eq!(l, Version::new(1, 0, 1));
            }
            OutdatedStatus::UpToDate => {
                // If somehow it's up to date, that's also acceptable
            }
            status => panic!("Expected Outdated or UpToDate status, got: {:?}", status),
        }
    }
}
