use crate::cache::Cache;
use crate::core::{LpmError, LpmResult};
use std::path::Path;

/// Calculate and record checksums for packages
pub struct ChecksumRecorder {
    cache: Cache,
}

impl ChecksumRecorder {
    pub fn new(cache: Cache) -> Self {
        Self { cache }
    }

    /// Calculate checksum for a downloaded source file
    ///
    /// This should be called after downloading a package source.
    /// The checksum is then recorded in the lockfile.
    pub fn calculate_for_source(&self, source_url: &str) -> LpmResult<String> {
        let source_path = self.cache.source_path(source_url);

        if !source_path.exists() {
            return Err(LpmError::Package(format!(
                "Source file not found: {}",
                source_path.display()
            )));
        }

        Cache::checksum(&source_path)
    }

    /// Calculate checksum for a file at a given path
    pub fn calculate_for_file(&self, file_path: &Path) -> LpmResult<String> {
        if !file_path.exists() {
            return Err(LpmError::Package(format!(
                "File not found: {}",
                file_path.display()
            )));
        }

        Cache::checksum(file_path)
    }

    /// Record checksum in lockfile after first install
    ///
    /// This is called during installation to ensure checksums are recorded
    /// in package.lock for future verification.
    pub fn record_checksum(&self, package_name: &str, source_url: &str) -> LpmResult<String> {
        let checksum = self.calculate_for_source(source_url)?;

        // Log for debugging
        eprintln!("📦 Calculated checksum for {}: {}", package_name, checksum);

        Ok(checksum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Cache;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_checksum_recorder_new() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let _recorder = ChecksumRecorder::new(cache);
        // Should create successfully
    }

    #[test]
    fn test_calculate_for_file() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let recorder = ChecksumRecorder::new(cache);

        let test_file = temp.path().join("test.txt");
        fs::write(&test_file, b"test data").unwrap();

        let checksum = recorder.calculate_for_file(&test_file).unwrap();
        assert!(checksum.starts_with("blake3:")); // Now defaults to BLAKE3
    }

    #[test]
    fn test_calculate_for_file_nonexistent() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let recorder = ChecksumRecorder::new(cache);

        let nonexistent = temp.path().join("nonexistent.txt");
        let result = recorder.calculate_for_file(&nonexistent);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_for_source_nonexistent() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        let recorder = ChecksumRecorder::new(cache);

        let result = recorder.calculate_for_source("https://example.com/nonexistent.tar.gz");
        assert!(result.is_err());
    }

    #[test]
    fn test_record_checksum() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf()).unwrap();
        cache.init().unwrap();
        let recorder = ChecksumRecorder::new(cache.clone());

        // Use a URL that would hash to a file
        let url = "https://example.com/test.tar.gz";
        let source_path = cache.source_path(url);
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(&source_path, b"test archive").unwrap();

        let checksum = recorder.record_checksum("test-package", url).unwrap();
        assert!(checksum.starts_with("blake3:")); // Now defaults to BLAKE3
    }
}
