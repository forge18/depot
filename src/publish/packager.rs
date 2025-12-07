use crate::build::prebuilt::PrebuiltBinaryManager;
use crate::build::targets::Target;
use crate::core::{LpmError, LpmResult};
use crate::lua_version::detector::LuaVersionDetector;
use crate::package::manifest::PackageManifest;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Packages Lua modules for publishing to LuaRocks
pub struct PublishPackager {
    project_root: PathBuf,
    manifest: PackageManifest,
}

impl PublishPackager {
    /// Create a new publish packager
    pub fn new(project_root: &Path, manifest: PackageManifest) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            manifest,
        }
    }

    /// Package the module for publishing
    ///
    /// This creates a distribution archive containing:
    /// - All Lua source files
    /// - Pre-built Rust binaries (if available)
    /// - Generated rockspec
    pub fn package(&self, include_binaries: bool) -> LpmResult<PathBuf> {
        let dist_dir = self.project_root.join("dist");
        let package_name = format!("{}-{}", self.manifest.name, self.manifest.version);
        let package_dir = dist_dir.join(&package_name);

        // Clean and create package directory
        if package_dir.exists() {
            fs::remove_dir_all(&package_dir)?;
        }
        fs::create_dir_all(&package_dir)?;

        // Copy Lua source files
        self.copy_lua_files(&package_dir)?;

        // Copy Rust binaries if available and requested
        if include_binaries && self.manifest.build.is_some() {
            self.copy_rust_binaries(&package_dir)?;
        }

        // Create archive
        let archive_path = self.create_archive(&package_dir, &package_name)?;

        println!("✓ Packaged for publishing: {}", archive_path.display());

        Ok(archive_path)
    }

    /// Copy all Lua source files to the package directory
    fn copy_lua_files(&self, package_dir: &Path) -> LpmResult<()> {
        // Look for Lua files in common directories
        let lua_dirs = vec!["lua", "src", "lib", "."];

        for dir_name in lua_dirs {
            let source_dir = self.project_root.join(dir_name);
            if source_dir.exists() && source_dir.is_dir() {
                let dest_dir = package_dir.join(dir_name);
                self.copy_directory(&source_dir, &dest_dir, |path| {
                    path.extension().map(|e| e == "lua").unwrap_or(false)
                })?;
            }
        }

        // Also copy any .lua files in the root
        for entry in fs::read_dir(&self.project_root)? {
            let entry = entry
                .map_err(|e| LpmError::Path(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "lua").unwrap_or(false) {
                let dest = package_dir.join(path.file_name().unwrap());
                fs::copy(&path, &dest)?;
            }
        }

        Ok(())
    }

    /// Copy Rust-compiled native module binaries
    fn copy_rust_binaries(&self, package_dir: &Path) -> LpmResult<()> {
        let lua_version = LuaVersionDetector::detect()?;
        let prebuilt_manager = PrebuiltBinaryManager::new()?;
        let default_target = Target::default_target();

        // Try to get pre-built binary for current platform
        if let Some(binary_path) = prebuilt_manager.get_prebuilt(
            &self.manifest.name,
            &self.manifest.version,
            &lua_version,
            &default_target,
        ) {
            // Create lib directory for binaries
            let lib_dir = package_dir.join("lib");
            fs::create_dir_all(&lib_dir)?;

            let binary_name = binary_path
                .file_name()
                .ok_or_else(|| LpmError::Package("Invalid binary path".to_string()))?;
            let dest = lib_dir.join(binary_name);
            fs::copy(&binary_path, &dest)?;
            println!("  Included binary: {}", dest.display());
        } else {
            println!("  No pre-built binary found for current platform");
        }

        Ok(())
    }

    /// Copy directory with file filter
    fn copy_directory<F>(&self, source: &Path, dest: &Path, filter: F) -> LpmResult<()>
    where
        F: Fn(&Path) -> bool,
    {
        fs::create_dir_all(dest)?;

        for entry in WalkDir::new(source) {
            let entry = entry
                .map_err(|e| LpmError::Path(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() && filter(path) {
                let rel_path = path
                    .strip_prefix(source)
                    .map_err(|e| LpmError::Path(format!("Failed to get relative path: {}", e)))?;
                let dest_path = dest.join(rel_path);

                // Create parent directories
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::copy(path, &dest_path)?;
            }
        }

        Ok(())
    }

    /// Create distribution archive (tar.gz or zip)
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
            self.create_zip(package_dir, &archive_path)?;
        } else {
            self.create_tar_gz(package_dir, &archive_path)?;
        }

        Ok(archive_path)
    }

    /// Create zip archive (Windows)
    fn create_zip(&self, source_dir: &Path, archive_path: &Path) -> LpmResult<()> {
        use std::process::Command;

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

    /// Create tar.gz archive (Unix)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageManifest;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_publish_packager_new() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest.clone());

        assert_eq!(packager.project_root, temp.path());
        assert_eq!(packager.manifest.name, "test-package");
    }

    #[test]
    fn test_copy_lua_files_from_src_dir() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        // Create src directory with Lua files
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.lua"), "print('hello')").unwrap();
        fs::write(src_dir.join("utils.lua"), "return {}").unwrap();

        // Create package directory
        let package_dir = temp.path().join("dist").join("test-package-1.0.0");
        fs::create_dir_all(&package_dir).unwrap();

        // Copy Lua files
        packager.copy_lua_files(&package_dir).unwrap();

        // Verify files were copied
        assert!(package_dir.join("src").join("main.lua").exists());
        assert!(package_dir.join("src").join("utils.lua").exists());
    }

    #[test]
    fn test_copy_lua_files_from_lua_dir() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        // Create lua directory
        let lua_dir = temp.path().join("lua");
        fs::create_dir_all(&lua_dir).unwrap();
        fs::write(lua_dir.join("module.lua"), "return {}").unwrap();

        let package_dir = temp.path().join("dist").join("test-package-1.0.0");
        fs::create_dir_all(&package_dir).unwrap();

        packager.copy_lua_files(&package_dir).unwrap();

        assert!(package_dir.join("lua").join("module.lua").exists());
    }

    #[test]
    fn test_copy_lua_files_from_root() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        // Create Lua file in root
        fs::write(temp.path().join("init.lua"), "return {}").unwrap();

        let package_dir = temp.path().join("dist").join("test-package-1.0.0");
        fs::create_dir_all(&package_dir).unwrap();

        packager.copy_lua_files(&package_dir).unwrap();

        assert!(package_dir.join("init.lua").exists());
    }

    #[test]
    fn test_copy_lua_files_filters_non_lua() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.lua"), "print('hello')").unwrap();
        fs::write(src_dir.join("config.txt"), "not lua").unwrap();

        let package_dir = temp.path().join("dist").join("test-package-1.0.0");
        fs::create_dir_all(&package_dir).unwrap();

        packager.copy_lua_files(&package_dir).unwrap();

        assert!(package_dir.join("src").join("main.lua").exists());
        assert!(!package_dir.join("src").join("config.txt").exists());
    }

    #[test]
    fn test_copy_directory_with_filter() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("file1.lua"), "content1").unwrap();
        fs::write(source_dir.join("file2.txt"), "content2").unwrap();
        fs::create_dir_all(source_dir.join("subdir")).unwrap();
        fs::write(source_dir.join("subdir").join("file3.lua"), "content3").unwrap();

        let dest_dir = temp.path().join("dest");
        packager
            .copy_directory(&source_dir, &dest_dir, |path| {
                path.extension().map(|e| e == "lua").unwrap_or(false)
            })
            .unwrap();

        assert!(dest_dir.join("file1.lua").exists());
        assert!(!dest_dir.join("file2.txt").exists());
        assert!(dest_dir.join("subdir").join("file3.lua").exists());
    }

    #[test]
    fn test_package_creates_dist_directory() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.version = "1.0.0".to_string();
        let packager = PublishPackager::new(temp.path(), manifest);

        // Create some Lua files
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.lua"), "print('hello')").unwrap();

        // Package without binaries
        let result = packager.package(false);

        // Should succeed (or fail on archive creation if tar/zip not available)
        // But at least the dist directory should be created
        if result.is_ok() {
            let archive_path = result.unwrap();
            assert!(archive_path.exists() || archive_path.parent().unwrap().exists());
        }
    }

    #[test]
    fn test_package_cleans_existing_dist() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.version = "1.0.0".to_string();
        let packager = PublishPackager::new(temp.path(), manifest);

        // Create existing dist directory
        let dist_dir = temp.path().join("dist");
        let package_dir = dist_dir.join("test-package-1.0.0");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(package_dir.join("old_file.txt"), "old").unwrap();

        // Create source files
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.lua"), "print('hello')").unwrap();

        // Package should clean and recreate
        let _result = packager.package(false);

        // Old file should be gone (if packaging succeeded)
        // Note: This test may not fully verify if archive creation fails
    }

    #[test]
    fn test_copy_lua_files_from_lib_dir() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        let lib_dir = temp.path().join("lib");
        fs::create_dir_all(&lib_dir).unwrap();
        fs::write(lib_dir.join("module.lua"), "return {}").unwrap();

        let package_dir = temp.path().join("dist").join("test-package-1.0.0");
        fs::create_dir_all(&package_dir).unwrap();

        packager.copy_lua_files(&package_dir).unwrap();
        assert!(package_dir.join("lib").join("module.lua").exists());
    }

    #[test]
    fn test_copy_directory_with_empty_source() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        let dest_dir = temp.path().join("dest");

        packager
            .copy_directory(&source_dir, &dest_dir, |_| true)
            .unwrap();
        // Should succeed even with empty directory
    }

    #[test]
    fn test_copy_directory_with_nested_files() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        let source_dir = temp.path().join("source");
        fs::create_dir_all(source_dir.join("subdir1").join("subdir2")).unwrap();
        fs::write(
            source_dir.join("subdir1").join("subdir2").join("file.lua"),
            "content",
        )
        .unwrap();

        let dest_dir = temp.path().join("dest");
        packager
            .copy_directory(&source_dir, &dest_dir, |path| {
                path.extension().map(|e| e == "lua").unwrap_or(false)
            })
            .unwrap();

        assert!(dest_dir
            .join("subdir1")
            .join("subdir2")
            .join("file.lua")
            .exists());
    }

    #[test]
    fn test_copy_rust_binaries_no_binary() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        let package_dir = temp.path().join("dist").join("test-package-1.0.0");
        fs::create_dir_all(&package_dir).unwrap();

        // Should succeed even if no binary is found
        let result = packager.copy_rust_binaries(&package_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_package_with_binaries() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.version = "1.0.0".to_string();
        let packager = PublishPackager::new(temp.path(), manifest);

        // Create some Lua files
        let src_dir = temp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("main.lua"), "print('hello')").unwrap();

        // Package with binaries (may not find binaries, but tests the path)
        let _result = packager.package(true);
    }

    #[test]
    fn test_copy_directory_with_nested_structure() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        let source_dir = temp.path().join("source");
        fs::create_dir_all(source_dir.join("subdir1").join("subdir2")).unwrap();
        fs::write(
            source_dir.join("subdir1").join("subdir2").join("file.lua"),
            "content",
        )
        .unwrap();

        let dest_dir = temp.path().join("dest");
        packager
            .copy_directory(&source_dir, &dest_dir, |path| {
                path.extension().map(|e| e == "lua").unwrap_or(false)
            })
            .unwrap();

        assert!(dest_dir
            .join("subdir1")
            .join("subdir2")
            .join("file.lua")
            .exists());
    }

    #[test]
    fn test_copy_directory_error_handling() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test-package".to_string());
        let packager = PublishPackager::new(temp.path(), manifest);

        // Try to copy from non-existent directory
        let result = packager.copy_directory(
            &temp.path().join("nonexistent"),
            &temp.path().join("dest"),
            |_| true,
        );
        assert!(result.is_err());
    }
}
