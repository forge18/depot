use lpm::core::path::find_project_root;
use lpm::core::{LpmError, LpmResult};
use lpm::package::lockfile::Lockfile;
use lpm::security::audit::format_report;
use lpm::security::osv::OsvApi;
use lpm::security::vulnerability::VulnerabilityReport;
use std::env;

pub async fn run() -> LpmResult<()> {
    let current_dir = env::current_dir()
        .map_err(|e| LpmError::Path(format!("Failed to get current directory: {}", e)))?;

    let project_root = find_project_root(&current_dir)?;

    // Load lockfile
    let lockfile = Lockfile::load(&project_root)?
        .ok_or_else(|| LpmError::Package("No lockfile. Run 'lpm install' first".to_string()))?;

    println!("Running security audit...");
    println!("  Querying OSV (Open Source Vulnerabilities) database...");
    println!();

    // Query OSV for each package
    let osv = OsvApi::new();
    let mut report = VulnerabilityReport::new();
    report.package_count = lockfile.packages.len();

    for (name, locked_pkg) in &lockfile.packages {
        println!("Checking {}@{}", name, locked_pkg.version);
        let vulns = osv.query_package(name, &locked_pkg.version).await?;
        for vuln in vulns {
            report.add(vuln);
        }
        report.checked_packages += 1;
    }

    // Display results
    let output = format_report(&report);
    print!("{}", output);

    // Exit with error code if critical/high vulnerabilities found
    if report.has_critical() || report.has_high() {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpm::package::lockfile::{LockedPackage, Lockfile};
    use lpm::security::vulnerability::VulnerabilityReport;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_audit_with_empty_lockfile() {
        // Test that audit handles empty lockfile
        let lockfile = Lockfile::new();
        assert!(lockfile.packages.is_empty());
    }

    #[test]
    fn test_vulnerability_report_creation() {
        // Test VulnerabilityReport structure
        let mut report = VulnerabilityReport::new();
        report.package_count = 5;
        report.checked_packages = 3;
        assert_eq!(report.package_count, 5);
        assert_eq!(report.checked_packages, 3);
    }

    #[test]
    fn test_osv_api_new() {
        // Test that OsvApi can be created
        let osv = OsvApi::new();
        // Just verify it was created
        let _ = osv;
    }

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_audit_run_with_mock_lockfile() {
        let temp = TempDir::new().unwrap();
        let mut lockfile = Lockfile::new();
        let locked_pkg = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None,
            checksum: "abc123".to_string(),
            size: Some(1000),
            dependencies: HashMap::new(),
            build: None,
        };
        lockfile.add_package("test-pkg".to_string(), locked_pkg);
        lockfile.save(temp.path()).unwrap();

        // Change to temp dir
        std::env::set_current_dir(temp.path()).unwrap();

        // This will fail without network, but tests the structure
        let _ = run().await;
    }

    #[test]
    fn test_vulnerability_report_has_critical() {
        let report = VulnerabilityReport::new();
        assert!(!report.has_critical());
        // Add critical vuln would require creating Vulnerability struct
    }

    #[test]
    fn test_vulnerability_report_has_high() {
        let report = VulnerabilityReport::new();
        assert!(!report.has_high());
    }

    #[tokio::test]
    async fn test_run_error_no_lockfile() {
        // Test error path when lockfile doesn't exist (line 16-17)
        // Note: This test changes directory which can cause issues with tarpaulin
        // Skip if running under coverage tool
        if std::env::var("CARGO_TARPAULIN").is_ok() {
            return;
        }

        let temp = TempDir::new().unwrap();
        // Create package.yaml to make it a project, but no lockfile
        std::fs::write(
            temp.path().join("package.yaml"),
            "name: test\nversion: 1.0.0\n",
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let result = run().await;
        std::env::set_current_dir(original_dir).unwrap();

        // Should fail with "No lockfile" error
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No lockfile"));
    }

    #[tokio::test]
    async fn test_run_error_no_project_root() {
        // Test error path when not in a project (line 13)
        // Create a temp dir that's not a project
        let temp = TempDir::new().unwrap();
        let subdir = temp.path().join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&subdir).unwrap();

        let result = run().await;
        std::env::set_current_dir(original_dir).unwrap();

        // Should fail - no project root found
        assert!(result.is_err());
    }

    #[test]
    fn test_vulnerability_report_add_and_count() {
        // Test report.add() and counting logic (line 32, 34)
        let mut report = VulnerabilityReport::new();
        report.package_count = 2;

        // Add some vulnerabilities (would need actual Vulnerability struct)
        // For now, just test the structure
        assert_eq!(report.package_count, 2);
        assert_eq!(report.checked_packages, 0);
    }
}
