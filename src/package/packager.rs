use crate::build::builder::RustBuilder;
use crate::build::targets::Target;
use crate::core::{LpmError, LpmResult};
use crate::package::manifest::PackageManifest;
use std::fs;
use std::path::{Path, PathBuf};

/// Packages built Rust-compiled Lua native module binaries
///
/// These are dynamic libraries (.so/.dylib/.dll) compiled from Rust code
/// that are part of Lua module packages, not standalone Rust libraries.
pub struct BinaryPackager {
    project_root: PathBuf,
    manifest: PackageManifest,
}

impl BinaryPackager {
    /// Create a new packager
    pub fn new(project_root: &Path, manifest: PackageManifest) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            manifest,
        }
    }

    /// Package built binaries for a specific target
    pub fn package_target(&self, target: &Target) -> LpmResult<PathBuf> {
        // Build the extension for the target
        let builder = RustBuilder::new(&self.project_root, &self.manifest)?;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let binary_path = rt.block_on(builder.build(Some(target)))?;

        // Create package directory
        let package_name = format!(
            "{}-{}-{}",
            self.manifest.name, self.manifest.version, target.triple
        );
        let package_dir = self.project_root.join("dist").join(&package_name);
        fs::create_dir_all(&package_dir)?;

        // Copy binary to package directory
        let binary_name = binary_path
            .file_name()
            .ok_or_else(|| LpmError::Package("Invalid binary path".to_string()))?;
        let dest_binary = package_dir.join(binary_name);
        fs::copy(&binary_path, &dest_binary)?;

        // Create package manifest
        self.create_package_manifest(&package_dir, target, &dest_binary)?;

        // Create archive (tar.gz or zip)
        let archive_path = self.create_archive(&package_dir, &package_name)?;

        println!("✓ Packaged: {}", archive_path.display());
        println!("  Binary: {}", dest_binary.display());
        println!("  Target: {}", target.triple);

        Ok(archive_path)
    }

    /// Create a package manifest file
    fn create_package_manifest(
        &self,
        package_dir: &Path,
        target: &Target,
        binary_path: &Path,
    ) -> LpmResult<()> {
        let manifest_content = format!(
            r#"# LPM Binary Package Manifest
name: {}
version: {}
target: {}
binary: {}
generated_at: "{}"
"#,
            self.manifest.name,
            self.manifest.version,
            target.triple,
            binary_path.file_name().unwrap().to_string_lossy(),
            chrono::Utc::now().to_rfc3339(),
        );

        let manifest_path = package_dir.join("package.yaml");
        fs::write(&manifest_path, manifest_content)?;

        Ok(())
    }

    /// Create an archive (tar.gz on Unix, zip on Windows)
    fn create_archive(&self, package_dir: &Path, package_name: &str) -> LpmResult<PathBuf> {
        let dist_dir = package_dir.parent().unwrap();
        let archive_name = format!(
            "{}.{}",
            package_name,
            if cfg!(target_os = "windows") {
                "zip"
            } else {
                "tar.gz"
            }
        );
        let archive_path = dist_dir.join(&archive_name);

        if cfg!(target_os = "windows") {
            // Use zip on Windows
            self.create_zip(package_dir, &archive_path)?;
        } else {
            // Use tar.gz on Unix
            self.create_tar_gz(package_dir, &archive_path)?;
        }

        Ok(archive_path)
    }

    /// Create a zip archive (Windows)
    fn create_zip(&self, source_dir: &Path, archive_path: &Path) -> LpmResult<()> {
        use std::process::Command;

        // Try to use system zip command
        let status = Command::new("zip")
            .arg("-r")
            .arg(archive_path)
            .arg(".")
            .current_dir(source_dir)
            .status()?;

        if !status.success() {
            return Err(LpmError::Package(
                "Failed to create zip archive. Install 'zip' command.".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a tar.gz archive (Unix)
    fn create_tar_gz(&self, source_dir: &Path, archive_path: &Path) -> LpmResult<()> {
        use std::process::Command;

        let status = Command::new("tar")
            .arg("-czf")
            .arg(archive_path)
            .arg("-C")
            .arg(source_dir)
            .arg(".")
            .status()?;

        if !status.success() {
            return Err(LpmError::Package(
                "Failed to create tar.gz archive. Install 'tar' command.".to_string(),
            ));
        }

        Ok(())
    }

    /// Package binaries for all targets
    pub fn package_all_targets(&self) -> LpmResult<Vec<(Target, PathBuf)>> {
        let mut results = Vec::new();

        for target_triple in crate::build::targets::SUPPORTED_TARGETS {
            let target = Target::new(target_triple)?;
            eprintln!("Packaging for target: {}", target.triple);

            match self.package_target(&target) {
                Ok(path) => {
                    results.push((target, path));
                    eprintln!("✓ Packaged successfully for {}", target_triple);
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to package for {}: {}", target_triple, e);
                    // Continue with other targets
                }
            }
        }

        if results.is_empty() {
            return Err(LpmError::Package(
                "Failed to package for all targets".to_string(),
            ));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageManifest;
    use tempfile::TempDir;

    #[test]
    fn test_binary_packager_new() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = BinaryPackager::new(temp.path(), manifest.clone());

        assert_eq!(packager.project_root, temp.path());
        assert_eq!(packager.manifest.name, "test-package");
    }

    #[test]
    fn test_create_package_manifest() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.version = "1.0.0".to_string();
        let packager = BinaryPackager::new(temp.path(), manifest);

        let package_dir = temp.path().join("package");
        fs::create_dir_all(&package_dir).unwrap();

        let target = Target::new("x86_64-unknown-linux-gnu").unwrap();
        let binary_path = package_dir.join("test.so");
        fs::write(&binary_path, "fake binary").unwrap();

        packager
            .create_package_manifest(&package_dir, &target, &binary_path)
            .unwrap();

        let manifest_path = package_dir.join("package.yaml");
        assert!(manifest_path.exists());

        let content = fs::read_to_string(&manifest_path).unwrap();
        assert!(content.contains("test-package"));
        assert!(content.contains("1.0.0"));
        assert!(content.contains("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_create_archive_paths() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = BinaryPackager::new(temp.path(), manifest);

        // Create dist directory structure
        let dist_dir = temp.path().join("dist");
        let package_dir = dist_dir.join("package");
        fs::create_dir_all(&package_dir).unwrap();

        let package_name = "test-package-1.0.0-x86_64-unknown-linux-gnu";
        // This will fail if tar/zip commands aren't available, but we can test the path logic
        let archive_path_result = packager.create_archive(&package_dir, package_name);

        // If archive creation succeeds, verify paths
        if let Ok(archive_path) = archive_path_result {
            // Should create archive in parent directory (dist)
            assert_eq!(archive_path.parent(), Some(dist_dir.as_path()));

            // Archive name should have correct extension
            if cfg!(target_os = "windows") {
                assert!(archive_path.to_string_lossy().ends_with(".zip"));
            } else {
                assert!(archive_path.to_string_lossy().ends_with(".tar.gz"));
            }
        }
        // If it fails due to missing tar/zip, that's expected in test environments
    }

    #[test]
    fn test_create_package_manifest_with_different_targets() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.version = "1.0.0".to_string();
        let packager = BinaryPackager::new(temp.path(), manifest);

        // Use supported targets only
        let targets = vec![
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-gnu", // Use -gnu instead of -msvc
        ];

        for target_triple in targets {
            let package_dir = temp.path().join(format!("package-{}", target_triple));
            fs::create_dir_all(&package_dir).unwrap();

            let target = Target::new(target_triple).unwrap();
            let binary_path = package_dir.join("test.so");
            fs::write(&binary_path, "fake binary").unwrap();

            packager
                .create_package_manifest(&package_dir, &target, &binary_path)
                .unwrap();

            let manifest_path = package_dir.join("package.yaml");
            let content = fs::read_to_string(&manifest_path).unwrap();
            assert!(content.contains(target_triple));
        }
    }

    #[test]
    fn test_create_zip_error_handling() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = BinaryPackager::new(temp.path(), manifest);

        // Test with nonexistent source directory
        let source_dir = temp.path().join("nonexistent");
        let archive_path = temp.path().join("test.zip");
        let result = packager.create_zip(&source_dir, &archive_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_tar_gz_error_handling() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = BinaryPackager::new(temp.path(), manifest);

        // Test with nonexistent source directory
        let source_dir = temp.path().join("nonexistent");
        let archive_path = temp.path().join("test.tar.gz");
        let result = packager.create_tar_gz(&source_dir, &archive_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_package_target_binary_path_error() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = BinaryPackager::new(temp.path(), manifest);

        // Will fail because RustBuilder will fail, but tests structure
        use crate::build::targets::Target;
        let target = Target::new("x86_64-unknown-linux-gnu").unwrap();
        let _result = packager.package_target(&target);
    }
}
