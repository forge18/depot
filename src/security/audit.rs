use crate::core::{DepotError, DepotResult};
use crate::package::lockfile::Lockfile;
use crate::security::advisory::AdvisoryDatabase;
use crate::security::osv::OsvApi;
use crate::security::vulnerability::{Severity, Vulnerability, VulnerabilityReport};
use std::path::Path;

/// Security auditor for checking package vulnerabilities
pub struct SecurityAuditor {
    advisory_db: AdvisoryDatabase,
}

impl SecurityAuditor {
    /// Create a new security auditor
    pub fn new() -> DepotResult<Self> {
        let advisory_db = AdvisoryDatabase::load()?;
        Ok(Self { advisory_db })
    }

    /// Create a new security auditor with OSV integration.
    ///
    /// Queries OSV for vulnerabilities in the provided packages (name + version pairs).
    pub async fn new_with_osv(osv: &OsvApi, packages: &[(String, String)]) -> DepotResult<Self> {
        let mut advisory_db = AdvisoryDatabase::load()?;

        for (name, version) in packages {
            match osv.query_package(name, version).await {
                Ok(vulns) => {
                    for vuln in vulns {
                        advisory_db.add_advisory(vuln);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to query OSV for {}: {}", name, e);
                }
            }
        }

        Ok(Self { advisory_db })
    }

    /// Run a security audit on the current project
    pub fn audit_project(project_root: &Path) -> DepotResult<VulnerabilityReport> {
        let auditor = Self::new()?;
        auditor.audit(project_root)
    }

    /// Run a security audit with OSV integration
    pub async fn audit_project_with_osv(project_root: &Path) -> DepotResult<VulnerabilityReport> {
        let lockfile =
            crate::package::lockfile::Lockfile::load(project_root)?.ok_or_else(|| {
                DepotError::Package(format!(
                    "No {} found. Run 'depot install' first.",
                    crate::package::lockfile::LOCKFILE_NAME
                ))
            })?;

        let packages: Vec<(String, String)> = lockfile
            .packages
            .iter()
            .map(|(name, info)| (name.clone(), info.version.clone()))
            .collect();

        let osv = OsvApi::new();
        let auditor = Self::new_with_osv(&osv, &packages).await?;
        auditor.audit(project_root)
    }

    /// Perform security audit
    fn audit(&self, project_root: &Path) -> DepotResult<VulnerabilityReport> {
        let lockfile = Lockfile::load(project_root)?.ok_or_else(|| {
            DepotError::Package(format!(
                "No {} found. Run 'depot install' first.",
                crate::package::lockfile::LOCKFILE_NAME
            ))
        })?;

        let mut report = VulnerabilityReport::new();
        report.package_count = lockfile.packages.len();

        for (package_name, package_info) in &lockfile.packages {
            report.checked_packages += 1;

            let vulnerabilities = self
                .advisory_db
                .check_package(package_name, &package_info.version);

            for vuln in vulnerabilities {
                report.add(vuln.clone());
            }
        }

        Ok(report)
    }

    /// Check a specific package for vulnerabilities
    pub fn check_package(&self, package: &str, version: &str) -> Vec<&Vulnerability> {
        self.advisory_db.check_package(package, version)
    }

    /// Get all known advisories for a package
    pub fn get_advisories(&self, package: &str) -> Vec<&Vulnerability> {
        self.advisory_db.get_advisories(package)
    }
}

/// Format vulnerability report for display
pub fn format_report(report: &VulnerabilityReport) -> String {
    use std::fmt::Write;

    let mut output = String::new();

    if report.is_empty() {
        let _ = writeln!(output, "✓ No known vulnerabilities found");
        let _ = writeln!(output, "  Checked {} package(s)", report.checked_packages);
        return output;
    }

    // Sort vulnerabilities by severity (critical first)
    let mut vulns = report.vulnerabilities.clone();
    vulns.sort_by(|a, b| b.severity.cmp(&a.severity));

    // Count by severity
    let counts = report.count_by_severity();

    let _ = writeln!(output, "\n🚨 Security Audit Results");
    let _ = writeln!(output, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let _ = writeln!(output, "Checked: {} package(s)", report.checked_packages);
    let _ = writeln!(
        output,
        "Found: {} vulnerability(ies)",
        report.vulnerabilities.len()
    );
    let _ = writeln!(output);

    // Summary by severity
    if let Some(count) = counts.get(&Severity::Critical) {
        let _ = writeln!(
            output,
            "  {} Critical: {}",
            Severity::Critical.emoji(),
            count
        );
    }
    if let Some(count) = counts.get(&Severity::High) {
        let _ = writeln!(output, "  {} High: {}", Severity::High.emoji(), count);
    }
    if let Some(count) = counts.get(&Severity::Medium) {
        let _ = writeln!(output, "  {} Medium: {}", Severity::Medium.emoji(), count);
    }
    if let Some(count) = counts.get(&Severity::Low) {
        let _ = writeln!(output, "  {} Low: {}", Severity::Low.emoji(), count);
    }

    let _ = writeln!(output);
    let _ = writeln!(output, "Vulnerabilities:");
    let _ = writeln!(output);

    // List each vulnerability
    for (i, vuln) in vulns.iter().enumerate() {
        let _ = writeln!(
            output,
            "{}. {} {} {}",
            i + 1,
            vuln.severity.emoji(),
            vuln.severity.as_str(),
            vuln.package
        );
        let _ = writeln!(
            output,
            "   Package: {}@{}",
            vuln.package, vuln.affected_versions
        );
        let _ = writeln!(output, "   Title: {}", vuln.title);

        if let Some(ref cve) = vuln.cve {
            let _ = writeln!(output, "   CVE: {}", cve);
        }

        if let Some(ref fixed_in) = vuln.fixed_in {
            let _ = writeln!(output, "   Fixed in: {}", fixed_in);
        }

        let _ = writeln!(output, "   Description: {}", vuln.description);

        if !vuln.references.is_empty() {
            let _ = writeln!(output, "   References:");
            for ref_link in &vuln.references {
                let _ = writeln!(output, "     - {}", ref_link);
            }
        }

        let _ = writeln!(output);
    }

    // Recommendations
    let _ = writeln!(output, "Recommendations:");
    if report.has_critical() || report.has_high() {
        let _ = writeln!(output, "  • Update vulnerable packages immediately");
        let _ = writeln!(output, "  • Review and test updates before deploying");
    } else {
        let _ = writeln!(output, "  • Consider updating packages to latest versions");
    }
    let _ = writeln!(output, "  • Run 'depot outdated' to see available updates");
    let _ = writeln!(
        output,
        "  • Run 'depot update <package>' to update specific packages"
    );

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::advisory::AdvisoryDatabase;
    use crate::security::osv::OsvApi;
    use crate::security::vulnerability::{Severity, Vulnerability};

    #[test]
    fn test_format_empty_report() {
        let report = VulnerabilityReport::new();
        let output = format_report(&report);
        assert!(output.contains("No known vulnerabilities"));
        assert!(output.contains("Checked 0 package(s)"));
    }

    #[test]
    fn test_format_report_with_vulnerabilities() {
        let mut report = VulnerabilityReport::new();
        report.checked_packages = 5;
        report.package_count = 5;

        let vuln = Vulnerability {
            package: "test-package".to_string(),
            affected_versions: "<2.0.0".to_string(),
            severity: Severity::Critical,
            cve: Some("CVE-2024-1234".to_string()),
            title: "Test Vulnerability".to_string(),
            description: "A test vulnerability".to_string(),
            fixed_in: Some("2.0.0".to_string()),
            references: vec!["https://example.com/advisory".to_string()],
        };
        report.add(vuln);

        let output = format_report(&report);
        assert!(output.contains("Security Audit Results"));
        assert!(output.contains("test-package"));
        assert!(output.contains("Critical"));
        assert!(output.contains("CVE-2024-1234"));
        assert!(output.contains("Fixed in: 2.0.0"));
    }

    #[test]
    fn test_security_auditor_check_package() {
        let mut db = AdvisoryDatabase::new();
        let vuln = Vulnerability {
            package: "test-package".to_string(),
            affected_versions: "<2.0.0".to_string(),
            severity: Severity::High,
            cve: None,
            title: "Test".to_string(),
            description: "Test".to_string(),
            fixed_in: Some("2.0.0".to_string()),
            references: Vec::new(),
        };
        db.add_advisory(vuln);

        let auditor = SecurityAuditor { advisory_db: db };
        let found = auditor.check_package("test-package", "1.0.0");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].package, "test-package");

        let not_found = auditor.check_package("test-package", "2.0.0");
        assert_eq!(not_found.len(), 0);
    }

    #[test]
    fn test_security_auditor_get_advisories() {
        let mut db = AdvisoryDatabase::new();
        let vuln1 = Vulnerability {
            package: "test-package".to_string(),
            affected_versions: "<2.0.0".to_string(),
            severity: Severity::High,
            cve: None,
            title: "Test 1".to_string(),
            description: "Test".to_string(),
            fixed_in: Some("2.0.0".to_string()),
            references: Vec::new(),
        };
        let vuln2 = Vulnerability {
            package: "test-package".to_string(),
            affected_versions: "<1.5.0".to_string(),
            severity: Severity::Medium,
            cve: None,
            title: "Test 2".to_string(),
            description: "Test".to_string(),
            fixed_in: Some("1.5.0".to_string()),
            references: Vec::new(),
        };
        db.add_advisory(vuln1);
        db.add_advisory(vuln2);

        let auditor = SecurityAuditor { advisory_db: db };
        let advisories = auditor.get_advisories("test-package");
        assert_eq!(advisories.len(), 2);
    }

    #[test]
    fn test_security_auditor_get_advisories_nonexistent() {
        let db = AdvisoryDatabase::new();
        let auditor = SecurityAuditor { advisory_db: db };
        let advisories = auditor.get_advisories("nonexistent-package");
        assert_eq!(advisories.len(), 0);
    }

    #[test]
    fn test_format_report_with_multiple_severities() {
        let mut report = VulnerabilityReport::new();
        report.checked_packages = 3;
        report.package_count = 3;

        let critical = Vulnerability {
            package: "critical-pkg".to_string(),
            affected_versions: "<1.0.0".to_string(),
            severity: Severity::Critical,
            cve: None,
            title: "Critical".to_string(),
            description: "Critical".to_string(),
            fixed_in: None,
            references: Vec::new(),
        };
        let high = Vulnerability {
            package: "high-pkg".to_string(),
            affected_versions: "<1.0.0".to_string(),
            severity: Severity::High,
            cve: None,
            title: "High".to_string(),
            description: "High".to_string(),
            fixed_in: None,
            references: Vec::new(),
        };
        let medium = Vulnerability {
            package: "medium-pkg".to_string(),
            affected_versions: "<1.0.0".to_string(),
            severity: Severity::Medium,
            cve: None,
            title: "Medium".to_string(),
            description: "Medium".to_string(),
            fixed_in: None,
            references: Vec::new(),
        };

        report.add(critical);
        report.add(high);
        report.add(medium);

        let output = format_report(&report);
        assert!(output.contains("Critical"));
        assert!(output.contains("High"));
        assert!(output.contains("Medium"));
    }

    #[test]
    fn test_format_report_with_references() {
        let mut report = VulnerabilityReport::new();
        report.checked_packages = 1;
        report.package_count = 1;

        let vuln = Vulnerability {
            package: "test-package".to_string(),
            affected_versions: "<2.0.0".to_string(),
            severity: Severity::High,
            cve: Some("CVE-2024-1234".to_string()),
            title: "Test Vulnerability".to_string(),
            description: "A test vulnerability".to_string(),
            fixed_in: Some("2.0.0".to_string()),
            references: vec![
                "https://example.com/advisory".to_string(),
                "https://example.com/cve".to_string(),
            ],
        };
        report.add(vuln);

        let output = format_report(&report);
        assert!(output.contains("References:"));
        assert!(output.contains("https://example.com/advisory"));
        assert!(output.contains("https://example.com/cve"));
    }

    #[test]
    fn test_format_report_without_cve() {
        let mut report = VulnerabilityReport::new();
        report.checked_packages = 1;
        report.package_count = 1;

        let vuln = Vulnerability {
            package: "test-package".to_string(),
            affected_versions: "<2.0.0".to_string(),
            severity: Severity::Medium,
            cve: None,
            title: "Test Vulnerability".to_string(),
            description: "A test vulnerability".to_string(),
            fixed_in: None,
            references: Vec::new(),
        };
        report.add(vuln);

        let output = format_report(&report);
        assert!(!output.contains("CVE:"));
    }

    #[test]
    fn test_format_report_recommendations_critical() {
        let mut report = VulnerabilityReport::new();
        report.checked_packages = 1;
        report.package_count = 1;

        let vuln = Vulnerability {
            package: "test-package".to_string(),
            affected_versions: "<2.0.0".to_string(),
            severity: Severity::Critical,
            cve: None,
            title: "Test".to_string(),
            description: "Test".to_string(),
            fixed_in: None,
            references: Vec::new(),
        };
        report.add(vuln);

        let output = format_report(&report);
        assert!(output.contains("Update vulnerable packages immediately"));
        assert!(output.contains("Review and test updates"));
    }

    #[test]
    fn test_format_report_recommendations_low() {
        let mut report = VulnerabilityReport::new();
        report.checked_packages = 1;
        report.package_count = 1;

        let vuln = Vulnerability {
            package: "test-package".to_string(),
            affected_versions: "<2.0.0".to_string(),
            severity: Severity::Low,
            cve: None,
            title: "Test".to_string(),
            description: "Test".to_string(),
            fixed_in: None,
            references: Vec::new(),
        };
        report.add(vuln);

        let output = format_report(&report);
        assert!(output.contains("Consider updating packages"));
        assert!(output.contains("depot outdated"));
    }

    #[tokio::test]
    async fn test_audit_project_with_osv_no_lockfile() {
        let temp = tempfile::TempDir::new().unwrap();
        // No lockfile
        let result = SecurityAuditor::audit_project_with_osv(temp.path()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No depot.lock"));
    }

    #[tokio::test]
    async fn test_new_with_osv_empty_packages() {
        let osv = OsvApi::new();
        let result = SecurityAuditor::new_with_osv(&osv, &[]).await;
        assert!(result.is_ok());
        let auditor = result.unwrap();
        let advisories = auditor.get_advisories("nonexistent");
        assert!(advisories.is_empty());
    }

    #[tokio::test]
    async fn test_new_with_osv_with_packages() {
        let osv = OsvApi::new();
        let packages = vec![("test-package".to_string(), "1.0.0".to_string())];
        let result = SecurityAuditor::new_with_osv(&osv, &packages).await;
        // May succeed or fail depending on network, but tests the path
        let _ = result;
    }

    #[test]
    fn test_audit_project_no_lockfile() {
        let temp = tempfile::TempDir::new().unwrap();
        let result = SecurityAuditor::audit_project(temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No depot.lock"));
    }

    #[test]
    fn test_audit_with_empty_lockfile() {
        let temp = tempfile::TempDir::new().unwrap();
        // Create empty lockfile using Lockfile::new() to ensure proper structure
        let lockfile = Lockfile::new();
        lockfile.save(temp.path()).unwrap();

        let auditor = SecurityAuditor::new().unwrap();
        let result = auditor.audit(temp.path());
        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(report.is_empty());
        assert_eq!(report.package_count, 0);
    }
}
