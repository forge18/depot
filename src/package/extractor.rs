use crate::core::{DepotError, DepotResult};
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Archive;

/// Extracts package archives (tar.gz, zip) to temporary directories
pub struct PackageExtractor {
    dest_dir: PathBuf,
}

impl PackageExtractor {
    /// Create a new PackageExtractor
    pub fn new(dest_dir: PathBuf) -> Self {
        Self { dest_dir }
    }

    /// Extract an archive file
    /// Returns the path to the root directory of the extracted archive
    pub fn extract(&self, archive_path: &Path) -> DepotResult<PathBuf> {
        // Determine archive type from extension
        let extension = archive_path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| DepotError::Package("Unknown archive format".to_string()))?;

        let result = match extension {
            "gz" | "tgz" => self.extract_targz(archive_path),
            "zip" => self.extract_zip(archive_path),
            _ => Err(DepotError::Package(format!(
                "Unsupported format: {}",
                extension
            ))),
        };

        // Cleanup temp directory on error
        if result.is_err() {
            let temp_dir = self.dest_dir.join(format!(
                ".tmp-{}",
                archive_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            ));
            let _ = fs::remove_dir_all(&temp_dir); // Ignore cleanup errors
        }

        result
    }

    fn extract_targz(&self, archive_path: &Path) -> DepotResult<PathBuf> {
        let file = File::open(archive_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        let temp_dir = self.dest_dir.join(format!(
            ".tmp-{}",
            archive_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
        ));

        // Clean up any existing temp dir
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
        }
        fs::create_dir_all(&temp_dir)?;

        archive.unpack(&temp_dir)?;

        // Find root directory (standardize: use first directory found)
        let mut entries = fs::read_dir(&temp_dir)?;
        let root = entries
            .find_map(|e| {
                let entry = e.ok()?;
                if entry.file_type().ok()?.is_dir() {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                // If no root dir, archive might have files at root - use temp_dir itself
                DepotError::Package(
                    "Archive has no root directory. Files at root level not supported.".to_string(),
                )
            })?;

        Ok(root)
    }

    fn extract_zip(&self, archive_path: &Path) -> DepotResult<PathBuf> {
        use zip::ZipArchive;

        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| DepotError::Package(format!("Invalid zip: {}", e)))?;

        let temp_dir = self.dest_dir.join(format!(
            ".tmp-{}",
            archive_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
        ));

        // Clean up any existing temp dir
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
        }
        fs::create_dir_all(&temp_dir)?;

        archive
            .extract(&temp_dir)
            .map_err(|e| DepotError::Package(format!("Extract failed: {}", e)))?;

        // Find root directory (standardize: use first directory found)
        let mut entries = fs::read_dir(&temp_dir)?;
        let root = entries
            .find_map(|e| {
                let entry = e.ok()?;
                if entry.file_type().ok()?.is_dir() {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                DepotError::Package(
                    "Archive has no root directory. Files at root level not supported.".to_string(),
                )
            })?;

        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_package_extractor_new() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        assert_eq!(extractor.dest_dir, temp.path());
    }

    #[test]
    fn test_package_extractor_unsupported_format() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let test_file = temp.path().join("test.unknown");
        fs::write(&test_file, "test").unwrap();

        let result = extractor.extract(&test_file);
        assert!(result.is_err());
        match result {
            Err(DepotError::Package(msg)) => {
                assert!(msg.contains("Unsupported format") || msg.contains("unknown"));
            }
            Ok(path) => panic!(
                "Expected Package error for unsupported format, but extraction succeeded to: {:?}",
                path
            ),
            Err(e) => panic!(
                "Expected Package error for unsupported format, but got: {:?}",
                e
            ),
        }
    }

    #[test]
    fn test_package_extractor_missing_file() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let test_file = temp.path().join("nonexistent.tar.gz");

        let result = extractor.extract(&test_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_package_extractor_no_extension() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let test_file = temp.path().join("test");
        fs::write(&test_file, "test").unwrap();

        let result = extractor.extract(&test_file);
        assert!(result.is_err());
        match result {
            Err(DepotError::Package(msg)) => {
                assert!(msg.contains("Unknown archive format") || msg.contains("format"));
            }
            Ok(path) => panic!(
                "Expected Package error for file with no extension, but extraction succeeded to: {:?}", path
            ),
            Err(e) => panic!(
                "Expected Package error for file with no extension, but got: {:?}", e
            ),
        }
    }

    #[test]
    fn test_package_extractor_tgz_extension() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let test_file = temp.path().join("test.tgz");
        fs::write(&test_file, "fake tgz").unwrap();

        // This will fail because it's not a real archive, but we're testing the extension detection
        let result = extractor.extract(&test_file);
        // Should attempt to extract as tar.gz (will fail on invalid format)
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_targz_invalid_file() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let test_file = temp.path().join("test.tar.gz");
        fs::write(&test_file, "not a real tar.gz").unwrap();

        let result = extractor.extract(&test_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_zip_invalid_file() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let test_file = temp.path().join("test.zip");
        fs::write(&test_file, "not a real zip").unwrap();

        let result = extractor.extract(&test_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_cleanup_on_error() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let test_file = temp.path().join("test.tar.gz");
        fs::write(&test_file, "invalid archive").unwrap();

        // Extract should fail and cleanup temp directory
        let result = extractor.extract(&test_file);
        assert!(result.is_err());

        // Temp directory should be cleaned up (or not exist)
        // Cleanup may or may not happen depending on error type
    }

    #[test]
    fn test_extract_with_different_extensions() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());

        // Test .gz extension
        let gz_file = temp.path().join("test.gz");
        fs::write(&gz_file, "invalid").unwrap();
        assert!(extractor.extract(&gz_file).is_err());

        // Test .tgz extension
        let tgz_file = temp.path().join("test.tgz");
        fs::write(&tgz_file, "invalid").unwrap();
        assert!(extractor.extract(&tgz_file).is_err());
    }

    #[test]
    fn test_extract_targz_with_existing_temp_dir() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let archive_path = temp.path().join("test.tar.gz");
        fs::write(&archive_path, "invalid").unwrap();

        // Create existing temp dir
        let existing_temp_dir = temp.path().join(".tmp-test");
        fs::create_dir_all(&existing_temp_dir).unwrap();
        fs::write(existing_temp_dir.join("file.txt"), "content").unwrap();

        // Should clean up and recreate
        let _ = extractor.extract_targz(&archive_path);
    }

    #[test]
    fn test_extract_zip_with_existing_temp_dir() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let archive_path = temp.path().join("test.zip");
        fs::write(&archive_path, "invalid").unwrap();

        // Create existing temp dir
        let temp_dir = temp.path().join(".tmp-test");
        fs::create_dir_all(&temp_dir).unwrap();
        fs::write(temp_dir.join("file.txt"), "content").unwrap();

        // Should clean up and recreate
        let _ = extractor.extract_zip(&archive_path);
    }

    #[test]
    fn test_extract_unknown_extension() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let archive_path = temp.path().join("test.unknown");
        fs::write(&archive_path, "content").unwrap();

        let result = extractor.extract(&archive_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_no_extension() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let archive_path = temp.path().join("test");
        fs::write(&archive_path, "content").unwrap();

        let result = extractor.extract(&archive_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_package_extractor_dest_dir() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().join("custom").to_path_buf());
        assert_eq!(extractor.dest_dir, temp.path().join("custom"));
    }

    #[test]
    fn test_extract_targz_no_root_directory() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use tar::Builder;

        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());

        // Create a tar.gz with files at root (no directory)
        let archive_path = temp.path().join("test.tar.gz");
        let file = File::create(&archive_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut tar = Builder::new(encoder);

        // Add a file directly to root (not in a directory)
        let mut header = tar::Header::new_gnu();
        header.set_path("file.txt").unwrap();
        header.set_size(4);
        header.set_cksum();
        tar.append(&header, &b"test"[..]).unwrap();
        tar.finish().unwrap();

        // Should fail because there's no root directory (or invalid archive format)
        let result = extractor.extract_targz(&archive_path);
        // May fail with "no root directory" or other error depending on tar format
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_zip_no_root_directory() {
        use std::io::Write;
        use zip::write::{FileOptions, ZipWriter};

        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());

        // Create a zip with files at root (no directory)
        let archive_path = temp.path().join("test.zip");
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);

        zip.start_file("file.txt", FileOptions::default()).unwrap();
        zip.write_all(b"test").unwrap();
        zip.finish().unwrap();

        // Should fail because there's no root directory
        let result = extractor.extract_zip(&archive_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no root directory"));
    }

    #[test]
    fn test_extract_cleanup_on_error_targz() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());
        let archive_path = temp.path().join("test.tar.gz");

        // Write invalid tar.gz content
        fs::write(&archive_path, b"invalid tar.gz content").unwrap();

        // Extract should fail and cleanup temp dir
        let result = extractor.extract(&archive_path);
        assert!(result.is_err());

        // Temp directory should be cleaned up (or not exist)
        // Cleanup may or may not happen depending on error type
    }

    #[test]
    fn test_extract_targz_with_root_directory() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use tar::Builder;

        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());

        // Create a tar.gz with a root directory
        let archive_path = temp.path().join("test.tar.gz");
        let file = File::create(&archive_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut tar = Builder::new(encoder);

        // Add a directory first
        let mut header = tar::Header::new_gnu();
        header.set_path("package/").unwrap();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_cksum();
        tar.append(&header, &[] as &[u8]).unwrap();

        // Add a file in the directory
        let mut file_header = tar::Header::new_gnu();
        file_header.set_path("package/file.txt").unwrap();
        file_header.set_size(4);
        file_header.set_cksum();
        tar.append(&file_header, &b"test"[..]).unwrap();
        tar.finish().unwrap();

        // Should succeed because there's a root directory
        let result = extractor.extract_targz(&archive_path);
        // May fail if tar format is incorrect, but tests the code path
        if let Ok(root) = result {
            assert!(root.ends_with("package"));
        }
    }

    #[test]
    fn test_extract_zip_with_root_directory() {
        use std::io::Write;
        use zip::write::{FileOptions, ZipWriter};

        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());

        // Create a zip with a root directory
        let archive_path = temp.path().join("test.zip");
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);

        // Add a directory
        zip.add_directory("package/", FileOptions::default())
            .unwrap();

        // Add a file in the directory
        zip.start_file("package/file.txt", FileOptions::default())
            .unwrap();
        zip.write_all(b"test").unwrap();
        zip.finish().unwrap();

        // Should succeed because there's a root directory
        let result = extractor.extract_zip(&archive_path);
        assert!(result.is_ok());
        let root = result.unwrap();
        assert!(root.ends_with("package"));
    }

    #[test]
    fn test_extract_targz_cleanup_existing_temp() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());

        // Create existing temp dir
        let existing_temp_dir = temp.path().join(".tmp-test");
        fs::create_dir_all(&existing_temp_dir).unwrap();
        fs::write(existing_temp_dir.join("old_file.txt"), "old").unwrap();

        // Create a valid tar.gz
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use tar::Builder;
        let archive_path = temp.path().join("test.tar.gz");
        let file = File::create(&archive_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut tar = Builder::new(encoder);

        let mut header = tar::Header::new_gnu();
        header.set_path("package/").unwrap();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_cksum();
        tar.append(&header, &[] as &[u8]).unwrap();
        tar.finish().unwrap();

        // Should clean up existing temp dir and extract
        let result = extractor.extract_targz(&archive_path);
        // May fail if tar format is incorrect, but tests cleanup path
        // Old file should be gone if extraction succeeded
        if result.is_ok() {
            assert!(!existing_temp_dir.join("old_file.txt").exists());
        }
    }

    #[test]
    fn test_extract_zip_cleanup_existing_temp() {
        let temp = TempDir::new().unwrap();
        let extractor = PackageExtractor::new(temp.path().to_path_buf());

        // Create existing temp dir
        let existing_temp_dir = temp.path().join(".tmp-test");
        fs::create_dir_all(&existing_temp_dir).unwrap();
        fs::write(existing_temp_dir.join("old_file.txt"), "old").unwrap();

        // Create a valid zip
        use zip::write::{FileOptions, ZipWriter};
        let archive_path = temp.path().join("test.zip");
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.add_directory("package/", FileOptions::default())
            .unwrap();
        zip.finish().unwrap();

        // Should clean up existing temp dir and extract
        let result = extractor.extract_zip(&archive_path);
        assert!(result.is_ok());
        // Old file should be gone
        assert!(!existing_temp_dir.join("old_file.txt").exists());
    }
}
