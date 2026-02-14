//! Package installation metadata

use crate::core::{DepotError, DepotResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Metadata for an installed package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub package_name: String,
    pub version: String,

    // GitHub source
    pub repository: String, // "owner/repo"
    pub ref_type: String,   // "release", "tag", "branch", "commit"
    pub ref_value: String,
    pub commit_sha: String,

    // Timestamps
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Native code info
    #[serde(default)]
    pub native_code_types: Vec<String>,
    #[serde(default)]
    pub prebuilt_binary: bool,
}

impl PackageMetadata {
    /// Create new metadata for a package
    pub fn new(
        package_name: String,
        version: String,
        repository: String,
        ref_type: String,
        ref_value: String,
        commit_sha: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            package_name,
            version,
            repository,
            ref_type,
            ref_value,
            commit_sha,
            installed_at: now,
            updated_at: now,
            native_code_types: Vec::new(),
            prebuilt_binary: false,
        }
    }

    /// Load metadata from a file
    pub fn load(path: &Path) -> DepotResult<Self> {
        if !path.exists() {
            return Err(DepotError::Package(format!(
                "Metadata file not found: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(path)?;
        let metadata: PackageMetadata = serde_yaml::from_str(&content)
            .map_err(|e| DepotError::Package(format!("Failed to parse metadata: {}", e)))?;

        Ok(metadata)
    }

    /// Save metadata to a file
    pub fn save(&self, path: &Path) -> DepotResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_yaml::to_string(self)
            .map_err(|e| DepotError::Package(format!("Failed to serialize metadata: {}", e)))?;

        fs::write(path, content)?;
        Ok(())
    }

    /// Update the updated_at timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Check if this is a fresh install (installed_at == updated_at within 1 second)
    pub fn is_fresh_install(&self) -> bool {
        (self.updated_at.timestamp() - self.installed_at.timestamp()).abs() <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_metadata() {
        let metadata = PackageMetadata::new(
            "test-pkg".to_string(),
            "1.0.0".to_string(),
            "owner/repo".to_string(),
            "release".to_string(),
            "v1.0.0".to_string(),
            "abc123".to_string(),
        );

        assert_eq!(metadata.package_name, "test-pkg");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.repository, "owner/repo");
        assert!(metadata.is_fresh_install());
    }

    #[test]
    fn test_save_and_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("metadata.yaml");

        let metadata = PackageMetadata::new(
            "test-pkg".to_string(),
            "1.0.0".to_string(),
            "owner/repo".to_string(),
            "release".to_string(),
            "v1.0.0".to_string(),
            "abc123".to_string(),
        );

        metadata.save(&path).unwrap();
        let loaded = PackageMetadata::load(&path).unwrap();

        assert_eq!(loaded.package_name, metadata.package_name);
        assert_eq!(loaded.version, metadata.version);
        assert_eq!(loaded.repository, metadata.repository);
    }

    #[test]
    fn test_load_nonexistent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nonexistent.yaml");

        let result = PackageMetadata::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_touch() {
        let mut metadata = PackageMetadata::new(
            "test-pkg".to_string(),
            "1.0.0".to_string(),
            "owner/repo".to_string(),
            "release".to_string(),
            "v1.0.0".to_string(),
            "abc123".to_string(),
        );

        let original_updated = metadata.updated_at;
        // Sleep for >2 seconds to ensure timestamp difference > 1 second
        // (timestamps are in whole seconds, so we need 2+ seconds wall-clock time)
        std::thread::sleep(std::time::Duration::from_millis(2100));
        metadata.touch();

        assert!(metadata.updated_at > original_updated);
        assert!(!metadata.is_fresh_install());
    }
}
