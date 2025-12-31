use crate::core::{LpmError, LpmResult};
use crate::package::lockfile::Lockfile;
use crate::security::advisory::AdvisoryDatabase;
use crate::security::vulnerability::{Severity, Vulnerability, VulnerabilityReport};
use std::path::Path;

/// Security auditor for checking package vulnerabilities
pub struct SecurityAuditor {
    advisory_db: AdvisoryDatabase,
}

impl SecurityAuditor {
    /// Create a new security auditor
    pub fn new() -> LpmResult<Self> {
        let advisory_db = AdvisoryDatabase::load()?;
        Ok(Self { advisory_db })
    }

    /// Create a new security auditor with OSV integration
    ///
    /// This will query OSV for vulnerabilities in the provided packages.
    pub async fn new_with_osv(packages: &[String]) -> LpmResult<Self> {
        let mut advisory_db = AdvisoryDatabase::load()?;

        // Load from OSV
        advisory_db.load_from_osv_batch(packages).await?;

        Ok(Self { advisory_db })
    }

    /// Run a security audit on the current project
    pub fn audit_project(project_root: &Path) -> LpmResult<VulnerabilityReport> {
        let auditor = Self::new()?;
        auditor.audit(project_root)
    }

    /// Run a security audit with OSV integration
    pub async fn audit_project_with_osv(project_root: &Path) -> LpmResult<VulnerabilityReport> {
        // Load lockfile to get package names
        let lockfile =
            crate::package::lockfile::Lockfile::load(project_root)?.ok_or_else(|| {
                LpmError::Package(format!(
                    "No {} found. Run 'lpm install' first.",
                    crate::package::lockfile::LOCKFILE_NAME
                ))
            })?;

        let package_names: Vec<String> = lockfile.packages.keys().cloned().collect();

        let auditor = Self::new_with_osv(&package_names).await?;
        auditor.audit(project_root)
    }

    /// Perform security audit
    fn audit(&self, project_root: &Path) -> LpmResult<VulnerabilityReport> {
        // Load lockfile to get installed packages
        let lockfile = Lockfile::load(project_root)?.ok_or_else(|| {
            LpmError::Package(format!(
                "No {} found. Run 'lpm install' first.",
                crate::package::lockfile::LOCKFILE_NAME
            ))
        })?;

        let mut report = VulnerabilityReport::new();
        report.package_count = lockfile.packages.len();

        // Check each package for vulnerabilities
        for (package_name, package_info) in &lockfile.packages {
            report.checked_packages += 1;

            // Check against advisory database
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
        writeln!(output, "✓ No known vulnerabilities found").unwrap();
        writeln!(output, "  Checked {} package(s)", report.checked_packages).unwrap();
        return output;
    }

    // Sort vulnerabilities by severity (critical first)
    let mut vulns = report.vulnerabilities.clone();
    vulns.sort_by(|a, b| b.severity.cmp(&a.severity));

    // Count by severity
    let counts = report.count_by_severity();

    writeln!(output, "\n🚨 Security Audit Results").unwrap();
    writeln!(output, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━").unwrap();
    writeln!(output, "Checked: {} package(s)", report.checked_packages).unwrap();
    writeln!(
        output,
        "Found: {} vulnerability(ies)",
        report.vulnerabilities.len()
    )
    .unwrap();
    writeln!(output).unwrap();

    // Summary by severity
    if let Some(count) = counts.get(&Severity::Critical) {
        writeln!(
            output,
            "  {} Critical: {}",
            Severity::Critical.emoji(),
            count
        )
        .unwrap();
    }
    if let Some(count) = counts.get(&Severity::High) {
        writeln!(output, "  {} High: {}", Severity::High.emoji(), count).unwrap();
    }
    if let Some(count) = counts.get(&Severity::Medium) {
        writeln!(output, "  {} Medium: {}", Severity::Medium.emoji(), count).unwrap();
    }
    if let Some(count) = counts.get(&Severity::Low) {
        writeln!(output, "  {} Low: {}", Severity::Low.emoji(), count).unwrap();
    }

    writeln!(output).unwrap();
    writeln!(output, "Vulnerabilities:").unwrap();
    writeln!(output).unwrap();

    // List each vulnerability
    for (i, vuln) in vulns.iter().enumerate() {
        writeln!(
            output,
            "{}. {} {} {}",
            i + 1,
            vuln.severity.emoji(),
            vuln.severity.as_str(),
            vuln.package
        )
        .unwrap();
        writeln!(
            output,
            "   Package: {}@{}",
            vuln.package, vuln.affected_versions
        )
        .unwrap();
        writeln!(output, "   Title: {}", vuln.title).unwrap();

        if let Some(ref cve) = vuln.cve {
            writeln!(output, "   CVE: {}", cve).unwrap();
        }

        if let Some(ref fixed_in) = vuln.fixed_in {
            writeln!(output, "   Fixed in: {}", fixed_in).unwrap();
        }

        writeln!(output, "   Description: {}", vuln.description).unwrap();

        if !vuln.references.is_empty() {
            writeln!(output, "   References:").unwrap();
            for ref_link in &vuln.references {
                writeln!(output, "     - {}", ref_link).unwrap();
            }
        }

        writeln!(output).unwrap();
    }

    // Recommendations
    writeln!(output, "Recommendations:").unwrap();
    if report.has_critical() || report.has_high() {
        writeln!(output, "  • Update vulnerable packages immediately").unwrap();
        writeln!(output, "  • Review and test updates before deploying").unwrap();
    } else {
        writeln!(output, "  • Consider updating packages to latest versions").unwrap();
    }
    writeln!(output, "  • Run 'lpm outdated' to see available updates").unwrap();
    writeln!(
        output,
        "  • Run 'lpm update <package>' to update specific packages"
    )
    .unwrap();

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::advisory::AdvisoryDatabase;
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
        assert!(output.contains("lpm outdated"));
    }

    #[tokio::test]
    async fn test_audit_project_with_osv_no_lockfile() {
        let temp = tempfile::TempDir::new().unwrap();
        // No lockfile
        let result = SecurityAuditor::audit_project_with_osv(temp.path()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No lpm.lock"));
    }

    #[tokio::test]
    async fn test_new_with_osv_empty_packages() {
        let result = SecurityAuditor::new_with_osv(&[]).await;
        assert!(result.is_ok());
        let auditor = result.unwrap();
        // Should have empty advisory database
        let advisories = auditor.get_advisories("nonexistent");
        assert!(advisories.is_empty());
    }

    #[tokio::test]
    async fn test_new_with_osv_with_packages() {
        // Test with mock packages (OSV API will be called)
        let result = SecurityAuditor::new_with_osv(&["test-package".to_string()]).await;
        // May succeed or fail depending on network, but tests the path
        let _ = result;
    }

    #[test]
    fn test_audit_project_no_lockfile() {
        let temp = tempfile::TempDir::new().unwrap();
        let result = SecurityAuditor::audit_project(temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No lpm.lock"));
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
