use crate::cache::Cache;
use crate::core::{LpmError, LpmResult};
use crate::package::lockfile::{LockedPackage, Lockfile};
use std::path::Path;

/// Verifies package checksums against the lockfile
pub struct PackageVerifier {
    cache: Cache,
}

impl PackageVerifier {
    pub fn new(cache: Cache) -> Self {
        Self { cache }
    }

    /// Verify all packages in the lockfile match their checksums
    pub fn verify_all(
        &self,
        lockfile: &Lockfile,
        project_root: &Path,
    ) -> LpmResult<VerificationResult> {
        let mut result = VerificationResult::new();

        for (name, package) in &lockfile.packages {
            match self.verify_package(name, package, project_root) {
                Ok(()) => result.add_success(name.clone()),
                Err(e) => result.add_failure(name.clone(), e.to_string()),
            }
        }

        Ok(result)
    }

    /// Verify a single package's checksum
    pub fn verify_package(
        &self,
        package_name: &str,
        package: &LockedPackage,
        _project_root: &Path,
    ) -> LpmResult<()> {
        // Extract checksum from lockfile (format: "sha256:...")
        let expected_checksum = &package.checksum;

        if !expected_checksum.starts_with("sha256:") {
            return Err(LpmError::Package(format!(
                "Invalid checksum format for '{}': expected 'sha256:...'",
                package_name
            )));
        }

        // Get the source file path from cache
        let source_path = if let Some(source_url) = &package.source_url {
            self.cache.source_path(source_url)
        } else {
            return Err(LpmError::Package(format!(
                "No source_url for package '{}' in lockfile",
                package_name
            )));
        };

        // Check if source file exists
        if !source_path.exists() {
            return Err(LpmError::Package(format!(
                "Source file not found for '{}': {}",
                package_name,
                source_path.display()
            )));
        }

        // Calculate actual checksum
        let actual_checksum = Cache::checksum(&source_path)?;

        // Compare checksums
        if actual_checksum != *expected_checksum {
            return Err(LpmError::Package(format!(
                "Checksum mismatch for '{}':\n  Expected: {}\n  Actual:   {}",
                package_name, expected_checksum, actual_checksum
            )));
        }

        Ok(())
    }

    /// Verify a package's checksum from a file path
    pub fn verify_file(&self, file_path: &Path, expected_checksum: &str) -> LpmResult<()> {
        if !expected_checksum.starts_with("sha256:") {
            return Err(LpmError::Package(
                "Invalid checksum format: expected 'sha256:...'".to_string(),
            ));
        }

        if !file_path.exists() {
            return Err(LpmError::Package(format!(
                "File not found: {}",
                file_path.display()
            )));
        }

        let actual_checksum = Cache::checksum(file_path)?;

        if actual_checksum != expected_checksum {
            return Err(LpmError::Package(format!(
                "Checksum mismatch:\n  Expected: {}\n  Actual:   {}",
                expected_checksum, actual_checksum
            )));
        }

        Ok(())
    }
}

/// Result of verification operation
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub successful: Vec<String>,
    pub failed: Vec<(String, String)>,
}

impl VerificationResult {
    pub fn new() -> Self {
        Self {
            successful: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub fn add_success(&mut self, package: String) {
        self.successful.push(package);
    }

    pub fn add_failure(&mut self, package: String, error: String) {
        self.failed.push((package, error));
    }

    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }

    pub fn total_verified(&self) -> usize {
        self.successful.len() + self.failed.len()
    }
}

impl Default for VerificationResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Cache;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_verify_file_success() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"test data").unwrap();

        let checksum = Cache::checksum(&test_file).unwrap();
        verifier.verify_file(&test_file, &checksum).unwrap();
    }

    #[test]
    fn test_verify_file_mismatch() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"test data").unwrap();

        let wrong_checksum =
            "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let result = verifier.verify_file(&test_file, wrong_checksum);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_file_invalid_checksum_format() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"test data").unwrap();

        let invalid_checksum = "invalid-format";
        let result = verifier.verify_file(&test_file, invalid_checksum);
        assert!(result.is_err());
        match result {
            Err(LpmError::Package(msg)) => {
                assert!(msg.contains("Invalid checksum format"));
            }
            _ => panic!("Expected Package error"),
        }
    }

    #[test]
    fn test_verify_file_missing_file() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let missing_file = temp.path().join("nonexistent.txt");
        let checksum = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let result = verifier.verify_file(&missing_file, checksum);
        assert!(result.is_err());
        match result {
            Err(LpmError::Package(msg)) => {
                assert!(msg.contains("File not found") || msg.contains("nonexistent"));
            }
            _ => panic!("Expected Package error"),
        }
    }

    #[test]
    fn test_verification_result_new() {
        let result = VerificationResult::new();
        assert!(result.successful.is_empty());
        assert!(result.failed.is_empty());
        assert!(result.is_success());
        assert_eq!(result.total_verified(), 0);
    }

    #[test]
    fn test_verification_result_default() {
        let result = VerificationResult::default();
        assert!(result.successful.is_empty());
        assert!(result.failed.is_empty());
    }

    #[test]
    fn test_verification_result_add_success() {
        let mut result = VerificationResult::new();
        result.add_success("package1".to_string());
        result.add_success("package2".to_string());

        assert_eq!(result.successful.len(), 2);
        assert_eq!(result.failed.len(), 0);
        assert!(result.is_success());
        assert_eq!(result.total_verified(), 2);
    }

    #[test]
    fn test_verification_result_add_failure() {
        let mut result = VerificationResult::new();
        result.add_failure("package1".to_string(), "error1".to_string());
        result.add_failure("package2".to_string(), "error2".to_string());

        assert_eq!(result.successful.len(), 0);
        assert_eq!(result.failed.len(), 2);
        assert!(!result.is_success());
        assert_eq!(result.total_verified(), 2);
        assert_eq!(result.failed[0].0, "package1");
        assert_eq!(result.failed[0].1, "error1");
    }

    #[test]
    fn test_verification_result_mixed() {
        let mut result = VerificationResult::new();
        result.add_success("package1".to_string());
        result.add_failure("package2".to_string(), "error".to_string());

        assert_eq!(result.successful.len(), 1);
        assert_eq!(result.failed.len(), 1);
        assert!(!result.is_success());
        assert_eq!(result.total_verified(), 2);
    }

    #[test]
    fn test_verify_package_success() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache.clone());

        // Create a test source file
        let source_url = "https://example.com/test.tar.gz";
        let source_path = cache.source_path(source_url);
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(&source_path, b"test data").unwrap();

        let checksum = Cache::checksum(&source_path).unwrap();
        let package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: Some(source_url.to_string()),
            checksum,
            size: None,
            dependencies: std::collections::HashMap::new(),
            build: None,
        };

        let result = verifier.verify_package("test-package", &package, temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_package_invalid_checksum_format() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: Some("https://example.com/test.tar.gz".to_string()),
            checksum: "invalid-format".to_string(),
            size: None,
            dependencies: std::collections::HashMap::new(),
            build: None,
        };

        let result = verifier.verify_package("test-package", &package, temp.path());
        assert!(result.is_err());
        match result {
            Err(LpmError::Package(msg)) => {
                assert!(msg.contains("Invalid checksum format"));
            }
            _ => panic!("Expected Package error"),
        }
    }

    #[test]
    fn test_verify_package_no_source_url() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None,
            checksum: "sha256:abc123".to_string(),
            size: None,
            dependencies: std::collections::HashMap::new(),
            build: None,
        };

        let result = verifier.verify_package("test-package", &package, temp.path());
        assert!(result.is_err());
        match result {
            Err(LpmError::Package(msg)) => {
                assert!(msg.contains("No source_url"));
            }
            _ => panic!("Expected Package error"),
        }
    }

    #[test]
    fn test_verify_all_empty_lockfile() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let lockfile = Lockfile::new();
        let result = verifier.verify_all(&lockfile, temp.path()).unwrap();
        assert!(result.is_success());
        assert_eq!(result.total_verified(), 0);
    }

    #[test]
    fn test_verify_package_source_file_not_found() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: Some("https://example.com/nonexistent.tar.gz".to_string()),
            checksum: "sha256:abc123".to_string(),
            size: None,
            dependencies: std::collections::HashMap::new(),
            build: None,
        };

        let result = verifier.verify_package("test-package", &package, temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_verify_all_with_mixed_results() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let verifier = PackageVerifier::new(cache);

        let mut lockfile = Lockfile::new();
        let package1 = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None, // Will fail
            checksum: "sha256:abc123".to_string(),
            size: None,
            dependencies: std::collections::HashMap::new(),
            build: None,
        };
        let package2 = LockedPackage {
            version: "2.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None, // Will fail
            checksum: "sha256:def456".to_string(),
            size: None,
            dependencies: std::collections::HashMap::new(),
            build: None,
        };

        lockfile.add_package("pkg1".to_string(), package1);
        lockfile.add_package("pkg2".to_string(), package2);

        let result = verifier.verify_all(&lockfile, temp.path()).unwrap();
        assert!(!result.is_success());
        assert_eq!(result.total_verified(), 2);
    }

    #[test]
    fn test_verification_result_total_verified_empty() {
        let result = VerificationResult::new();
        assert_eq!(result.total_verified(), 0);
    }

    #[test]
    fn test_verification_result_is_success_with_failures() {
        let mut result = VerificationResult::new();
        result.add_success("pkg1".to_string());
        result.add_failure("pkg2".to_string(), "error".to_string());
        assert!(!result.is_success());
    }
}
