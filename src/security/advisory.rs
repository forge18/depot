use crate::core::DepotResult;
use crate::security::vulnerability::Vulnerability;
use std::collections::HashMap;

/// Database of security advisories
pub struct AdvisoryDatabase {
    advisories: HashMap<String, Vec<Vulnerability>>,
}

impl AdvisoryDatabase {
    /// Create a new advisory database
    pub fn new() -> Self {
        Self {
            advisories: HashMap::new(),
        }
    }

    /// Load advisories from a source
    pub fn load() -> DepotResult<Self> {
        Ok(Self::new())
    }

    /// Add an advisory to the database
    pub fn add_advisory(&mut self, vuln: Vulnerability) {
        self.advisories
            .entry(vuln.package.clone())
            .or_default()
            .push(vuln);
    }

    /// Check a package version for vulnerabilities
    pub fn check_package(&self, package: &str, version: &str) -> Vec<&Vulnerability> {
        let mut found = Vec::new();

        if let Some(advisories) = self.advisories.get(package) {
            for vuln in advisories {
                if vuln.affects_version(version) {
                    found.push(vuln);
                }
            }
        }

        found
    }

    /// Get all advisories for a package (regardless of version)
    pub fn get_advisories(&self, package: &str) -> Vec<&Vulnerability> {
        self.advisories
            .get(package)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a package has any known vulnerabilities
    pub fn has_vulnerabilities(&self, package: &str) -> bool {
        self.advisories.contains_key(package)
    }
}

impl Default for AdvisoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::vulnerability::Severity;

    #[test]
    fn test_advisory_database() {
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

        let found = db.check_package("test-package", "1.0.0");
        assert_eq!(found.len(), 1);

        let found = db.check_package("test-package", "2.0.0");
        assert_eq!(found.len(), 0);
    }

    #[test]
    fn test_advisory_database_new() {
        let db = AdvisoryDatabase::new();
        assert!(db.advisories.is_empty());
    }

    #[test]
    fn test_advisory_database_add_multiple_advisories() {
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

        let found = db.check_package("test-package", "1.0.0");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_advisory_database_get_advisories() {
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

        let advisories = db.get_advisories("test-package");
        assert_eq!(advisories.len(), 1);

        let nonexistent = db.get_advisories("nonexistent");
        assert_eq!(nonexistent.len(), 0);
    }

    #[test]
    fn test_advisory_database_has_vulnerabilities() {
        let mut db = AdvisoryDatabase::new();

        assert!(!db.has_vulnerabilities("test-package"));

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
        assert!(db.has_vulnerabilities("test-package"));
        assert!(!db.has_vulnerabilities("other-package"));
    }

    #[test]
    fn test_advisory_database_check_package_nonexistent() {
        let db = AdvisoryDatabase::new();
        let found = db.check_package("nonexistent-package", "1.0.0");
        assert_eq!(found.len(), 0);
    }

    #[test]
    fn test_advisory_database_default() {
        let db = AdvisoryDatabase::default();
        assert!(db.advisories.is_empty());
    }

    #[test]
    fn test_advisory_database_load() {
        let db = AdvisoryDatabase::load().unwrap();
        assert!(db.advisories.is_empty());
    }
}
