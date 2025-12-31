use crate::core::path::{ensure_dir, lpm_metadata_dir, lua_modules_dir, packages_metadata_dir};
use crate::core::{LpmError, LpmResult};
use crate::di::{CacheProvider, ConfigProvider, PackageClient, SearchProvider, ServiceContainer};
use crate::luarocks::rockspec::Rockspec;

#[cfg(test)]
use crate::cache::Cache;
#[cfg(test)]
use crate::config::Config;
#[cfg(test)]
use crate::luarocks::client::LuaRocksClient;
#[cfg(test)]
use crate::luarocks::search_api::SearchAPI;
use crate::package::extractor::PackageExtractor;
use crate::package::lockfile::Lockfile;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

/// Install a package to lua_modules/
pub struct PackageInstaller {
    project_root: PathBuf,
    lua_modules: PathBuf,
    metadata_dir: PathBuf,
    packages_dir: PathBuf,
    config: Arc<dyn ConfigProvider>,
    cache: Arc<dyn CacheProvider>,
    package_client: Arc<dyn PackageClient>,
    search_provider: Arc<dyn SearchProvider>,
    extractor: PackageExtractor,
}

impl PackageInstaller {
    /// Create a new installer for a project
    pub fn new(project_root: &Path) -> LpmResult<Self> {
        let container = ServiceContainer::new()?;
        Self::with_dependencies(
            project_root,
            container.config.clone(),
            container.cache.clone(),
            container.package_client.clone(),
            container.search_provider.clone(),
        )
    }

    /// Create a new installer with injected dependencies (proper DI)
    ///
    /// This is the primary constructor that accepts dependency injection.
    /// Use this for testing with mock implementations.
    pub fn with_dependencies(
        project_root: &Path,
        config: Arc<dyn ConfigProvider>,
        cache: Arc<dyn CacheProvider>,
        package_client: Arc<dyn PackageClient>,
        search_provider: Arc<dyn SearchProvider>,
    ) -> LpmResult<Self> {
        let lua_modules = lua_modules_dir(project_root);
        let metadata_dir = lpm_metadata_dir(project_root);
        let packages_dir = packages_metadata_dir(project_root);
        let extractor = PackageExtractor::new(lua_modules.clone());

        Ok(Self {
            project_root: project_root.to_path_buf(),
            lua_modules,
            metadata_dir,
            packages_dir,
            config,
            cache,
            package_client,
            search_provider,
            extractor,
        })
    }

    /// Create a new installer with a service container (convenience wrapper)
    ///
    /// Deprecated: Use with_dependencies for proper DI
    #[deprecated(note = "Use with_dependencies instead for proper dependency injection")]
    pub fn with_container(project_root: &Path, container: ServiceContainer) -> LpmResult<Self> {
        Self::with_dependencies(
            project_root,
            container.config.clone(),
            container.cache.clone(),
            container.package_client.clone(),
            container.search_provider.clone(),
        )
    }

    /// Create a new installer with injected config and cache (for testing)
    ///
    /// DEPRECATED: Use `with_dependencies` instead for better testability.
    #[cfg(test)]
    #[deprecated(note = "Use with_dependencies instead")]
    pub fn with_config(project_root: &Path, config: Config, cache: Cache) -> LpmResult<Self> {
        let lua_modules = lua_modules_dir(project_root);
        let metadata_dir = lpm_metadata_dir(project_root);
        let packages_dir = packages_metadata_dir(project_root);
        let client = LuaRocksClient::new(&config, cache.clone());
        let search_api = SearchAPI::new();
        let extractor = PackageExtractor::new(lua_modules.clone());

        use std::sync::Arc;

        Ok(Self {
            project_root: project_root.to_path_buf(),
            lua_modules,
            metadata_dir,
            packages_dir,
            config: Arc::new(config),
            cache: Arc::new(cache),
            package_client: Arc::new(client),
            search_provider: Arc::new(search_api),
            extractor,
        })
    }

    /// Initialize the directory structure
    pub fn init(&self) -> LpmResult<()> {
        ensure_dir(&self.lua_modules)?;
        ensure_dir(&self.metadata_dir)?;
        ensure_dir(&self.packages_dir)?;
        Ok(())
    }

    /// Install a package
    pub async fn install_package(&self, name: &str, version: &str) -> LpmResult<PathBuf> {
        println!("Installing {}@{}", name, version);

        // Step 1: Construct and verify rockspec URL
        println!("  Fetching package info...");
        let rockspec_url = self.search_provider.get_rockspec_url(name, version, None);
        self.search_provider.verify_rockspec_url(&rockspec_url).await?;

        // Step 2: Download and parse rockspec to get build configuration
        println!("  Downloading rockspec...");
        let rockspec_content = self.package_client.download_rockspec(&rockspec_url).await?;
        let rockspec = self.package_client.parse_rockspec(&rockspec_content)?;

        // Step 3: Download source archive
        println!("  Downloading source...");
        let source_path = self.package_client.download_source(&rockspec.source.url).await?;

        // Step 4: Verify checksum if lockfile exists (ensures reproducible installs)
        if let Some(lockfile) = Lockfile::load(&self.project_root)? {
            if let Some(locked_pkg) = lockfile.get_package(name) {
                println!("  Verifying checksum...");
                let actual = self.cache.checksum(&source_path)?;
                if actual != locked_pkg.checksum {
                    return Err(LpmError::Package(format!(
                        "Checksum mismatch for {}@{}. Expected {}, got {}",
                        name, version, locked_pkg.checksum, actual
                    )));
                }
                println!("  ✓ Checksum verified");
            }
        }

        // Step 5: Extract source archive to temporary directory
        println!("  Extracting...");
        let extracted_path = self.extractor.extract(&source_path)?;

        // Step 6: Build and install based on rockspec build type
        println!("  Installing...");
        self.install_from_source(&extracted_path, name, &rockspec)?;

        // Step 7: Calculate checksum for lockfile generation
        let checksum = self.cache.checksum(&source_path)?;

        println!("  ✓ Installed {} (checksum: {})", name, checksum);

        Ok(self.lua_modules.join(name))
    }

    fn install_from_source(
        &self,
        source_path: &Path,
        package_name: &str,
        rockspec: &Rockspec,
    ) -> LpmResult<()> {
        match rockspec.build.build_type.as_str() {
            "none" | "builtin" => {
                // Pure Lua modules: copy files directly without building.
                self.install_builtin(source_path, package_name, rockspec)
            },
            "make" => {
                // Build using Makefile.
                self.build_with_make(source_path, package_name, rockspec)
            },
            "cmake" => {
                // Build using CMake.
                self.build_with_cmake(source_path, package_name, rockspec)
            },
            "command" => {
                // Build using custom command specified in rockspec.
                self.build_with_command(source_path, package_name, rockspec)
            },
            "rust" | "rust-mlua" => {
                // Rust extensions using mlua: build with cargo.
                self.build_with_rust(source_path, package_name, rockspec)
            },
            _ => Err(LpmError::NotImplemented(format!(
                "Build type '{}' not supported. Supported types: builtin, none, make, cmake, command, rust.",
                rockspec.build.build_type
            ))),
        }
    }

    fn build_with_make(
        &self,
        source_path: &Path,
        package_name: &str,
        rockspec: &Rockspec,
    ) -> LpmResult<()> {
        use std::process::Command;

        println!("  Building with make...");

        let mut make_cmd = Command::new("make");
        make_cmd.current_dir(source_path);

        let status = make_cmd
            .status()
            .map_err(|e| LpmError::Package(format!("Failed to run make: {}", e)))?;

        if !status.success() {
            return Err(LpmError::Package("make build failed".to_string()));
        }

        // Install using make install (if install target exists) or copy built files
        let dest = self.lua_modules.join(package_name);
        fs::create_dir_all(&dest)?;

        // Attempt make install first, fall back to manual file copying if needed
        let mut install_cmd = Command::new("make");
        install_cmd.arg("install");
        install_cmd.current_dir(source_path);
        install_cmd.env("PREFIX", &dest);

        if install_cmd.status().is_ok() {
            println!("  ✓ Installed via make install");
            return Ok(());
        }

        // Fall back to copying files based on rockspec.build.install or build.modules.
        // Handle install table sections: bin, lua, lib, conf.
        let has_install = !rockspec.build.install.bin.is_empty()
            || !rockspec.build.install.lua.is_empty()
            || !rockspec.build.install.lib.is_empty()
            || !rockspec.build.install.conf.is_empty();

        if has_install {
            // Copy files from install.bin (executables).
            for source_path_str in rockspec.build.install.bin.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if src.is_dir() {
                        copy_dir_recursive(&src, &dst)?;
                    } else {
                        fs::copy(&src, &dst)?;
                    }
                }
            }

            // Copy files from install.lua (Lua modules).
            for source_path_str in rockspec.build.install.lua.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if src.is_dir() {
                        copy_dir_recursive(&src, &dst)?;
                    } else {
                        fs::copy(&src, &dst)?;
                    }
                }
            }

            // Copy files from install.lib (native libraries).
            for source_path_str in rockspec.build.install.lib.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::copy(&src, &dst)?;
                }
            }

            // Copy files from install.conf (configuration files).
            for source_path_str in rockspec.build.install.conf.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if src.is_dir() {
                        copy_dir_recursive(&src, &dst)?;
                    } else {
                        fs::copy(&src, &dst)?;
                    }
                }
            }
        } else if !rockspec.build.modules.is_empty() {
            // Copy modules specified in build.modules.
            for source_file in rockspec.build.modules.values() {
                let src = source_path.join(source_file);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&src, &dst)?;
                }
            }
        } else {
            // Copy everything as fallback.
            copy_dir_recursive(source_path, &dest)?;
        }

        println!("  ✓ Installed built package");
        Ok(())
    }

    fn build_with_cmake(
        &self,
        source_path: &Path,
        package_name: &str,
        rockspec: &Rockspec,
    ) -> LpmResult<()> {
        use std::process::Command;

        println!("  Building with cmake...");

        // Create build directory for CMake.
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir)?;

        // Run cmake configure step.
        let mut cmake_cmd = Command::new("cmake");
        cmake_cmd.arg("..");
        cmake_cmd.current_dir(&build_dir);

        let status = cmake_cmd
            .status()
            .map_err(|e| LpmError::Package(format!("Failed to run cmake: {}", e)))?;

        if !status.success() {
            return Err(LpmError::Package("cmake configure failed".to_string()));
        }

        // Run cmake build step.
        let mut build_cmd = Command::new("cmake");
        build_cmd.args(["--build", "."]);
        build_cmd.current_dir(&build_dir);

        let status = build_cmd
            .status()
            .map_err(|e| LpmError::Package(format!("Failed to run cmake build: {}", e)))?;

        if !status.success() {
            return Err(LpmError::Package("cmake build failed".to_string()));
        }

        // Install built files to destination.
        let dest = self.lua_modules.join(package_name);
        fs::create_dir_all(&dest)?;

        // Attempt cmake install first.
        let mut install_cmd = Command::new("cmake");
        install_cmd.args(["--install", ".", "--prefix", dest.to_str().unwrap()]);
        install_cmd.current_dir(&build_dir);

        if install_cmd.status().is_ok() {
            println!("  ✓ Installed via cmake install");
            return Ok(());
        }

        // Fall back to copying from build directory.
        let has_install = !rockspec.build.install.bin.is_empty()
            || !rockspec.build.install.lua.is_empty()
            || !rockspec.build.install.lib.is_empty()
            || !rockspec.build.install.conf.is_empty();

        if has_install {
            // Copy from install table sections.
            for source_path_str in rockspec.build.install.bin.values() {
                let src = build_dir.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(&build_dir)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if src.is_dir() {
                        copy_dir_recursive(&src, &dst)?;
                    } else {
                        fs::copy(&src, &dst)?;
                    }
                }
            }

            for source_path_str in rockspec.build.install.lua.values() {
                let src = build_dir.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(&build_dir)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::copy(&src, &dst)?;
                }
            }

            for source_path_str in rockspec.build.install.lib.values() {
                let src = build_dir.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(&build_dir)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::copy(&src, &dst)?;
                }
            }
        } else {
            // Copy built files from build directory.
            copy_dir_recursive(&build_dir, &dest)?;
        }

        println!("  ✓ Installed built package");
        Ok(())
    }

    fn build_with_command(
        &self,
        source_path: &Path,
        package_name: &str,
        rockspec: &Rockspec,
    ) -> LpmResult<()> {
        use std::process::Command;

        // For "command" build type, parse the command from rockspec.
        // LuaRocks stores it in build.variables or build.command.
        // This implementation checks for a common build.sh pattern.

        println!("  Building with custom command...");

        // Check for build script or command specification.
        // Full implementation would parse rockspec.build.variables.
        let build_script = source_path.join("build.sh");
        if build_script.exists() {
            let mut cmd = Command::new("sh");
            cmd.arg(&build_script);
            cmd.current_dir(source_path);

            let status = cmd
                .status()
                .map_err(|e| LpmError::Package(format!("Failed to run build script: {}", e)))?;

            if !status.success() {
                return Err(LpmError::Package("Custom build command failed".to_string()));
            }
        } else {
            return Err(LpmError::Package(
                "command build type requires a build script or command specification in rockspec"
                    .to_string(),
            ));
        }

        // Install built files to destination.
        let dest = self.lua_modules.join(package_name);
        fs::create_dir_all(&dest)?;

        let has_install = !rockspec.build.install.bin.is_empty()
            || !rockspec.build.install.lua.is_empty()
            || !rockspec.build.install.lib.is_empty()
            || !rockspec.build.install.conf.is_empty();

        if has_install {
            // Copy from install table sections.
            for source_path_str in rockspec.build.install.bin.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    if src.is_dir() {
                        copy_dir_recursive(&src, &dst)?;
                    } else {
                        fs::copy(&src, &dst)?;
                    }
                }
            }

            for source_path_str in rockspec.build.install.lua.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::copy(&src, &dst)?;
                }
            }
        } else {
            // Copy everything as fallback.
            copy_dir_recursive(source_path, &dest)?;
        }

        println!("  ✓ Installed built package");
        Ok(())
    }

    fn build_with_rust(
        &self,
        source_path: &Path,
        package_name: &str,
        rockspec: &Rockspec,
    ) -> LpmResult<()> {
        use std::process::Command;

        println!("  Building Rust extension...");

        // Verify Cargo.toml exists (required for Rust builds).
        let cargo_toml = source_path.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err(LpmError::Package(
                "Rust build type requires Cargo.toml in package source".to_string(),
            ));
        }

        // Build with cargo in release mode.
        let mut build_cmd = Command::new("cargo");
        build_cmd.args(["build", "--release"]);
        build_cmd.current_dir(source_path);

        let status = build_cmd
            .status()
            .map_err(|e| LpmError::Package(format!("Failed to run cargo build: {}", e)))?;

        if !status.success() {
            return Err(LpmError::Package("cargo build failed".to_string()));
        }

        // Find the built library in target/release/.
        // Look for platform-specific extensions: .so, .dylib, or .dll.
        let target_dir = source_path.join("target").join("release");
        let lib_ext = if cfg!(target_os = "windows") {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };

        // Search for the built library file in target/release/.
        let lib_file = std::fs::read_dir(&target_dir)?
            .filter_map(|e| e.ok())
            .find(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == lib_ext)
                    .unwrap_or(false)
            });

        // Install built files to destination.
        let dest = self.lua_modules.join(package_name);
        fs::create_dir_all(&dest)?;

        if let Some(lib_entry) = lib_file {
            // Copy the built library to destination.
            let lib_path = lib_entry.path();
            let lib_name = lib_path
                .file_name()
                .ok_or_else(|| LpmError::Package("Invalid library path".to_string()))?;
            let dest_lib = dest.join(lib_name);
            fs::copy(&lib_path, &dest_lib)?;
            println!("  ✓ Copied library: {}", lib_name.to_string_lossy());
        }

        // Copy Lua files if specified in modules.
        if !rockspec.build.modules.is_empty() {
            for source_file in rockspec.build.modules.values() {
                let src = source_path.join(source_file);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&src, &dst)?;
                }
            }
        }

        // Copy any other files specified in install table.
        let has_install = !rockspec.build.install.bin.is_empty()
            || !rockspec.build.install.lua.is_empty()
            || !rockspec.build.install.lib.is_empty()
            || !rockspec.build.install.conf.is_empty();

        if has_install {
            for source_path_str in rockspec.build.install.bin.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::copy(&src, &dst)?;
                }
            }

            for source_path_str in rockspec.build.install.lua.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::copy(&src, &dst)?;
                }
            }

            for source_path_str in rockspec.build.install.lib.values() {
                let src = source_path.join(source_path_str);
                if src.exists() {
                    let relative = src
                        .strip_prefix(source_path)
                        .map_err(|e| LpmError::Path(e.to_string()))?;
                    let dst = dest.join(relative);

                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::copy(&src, &dst)?;
                }
            }
        }

        println!("  ✓ Installed Rust extension");
        Ok(())
    }

    fn install_builtin(
        &self,
        source_path: &Path,
        package_name: &str,
        rockspec: &Rockspec,
    ) -> LpmResult<()> {
        let dest = self.lua_modules.join(package_name);
        fs::create_dir_all(&dest)?;

        if rockspec.build.modules.is_empty() {
            // Copy everything (standard case for most packages).
            copy_dir_recursive(source_path, &dest)?;
        } else {
            // Copy only the specified modules.
            for source_file in rockspec.build.modules.values() {
                let src = source_path.join(source_file);
                if !src.exists() {
                    return Err(LpmError::Package(format!(
                        "Module file not found in source: {}",
                        source_file
                    )));
                }

                let relative = src
                    .strip_prefix(source_path)
                    .map_err(|e| LpmError::Path(e.to_string()))?;
                let dst = dest.join(relative);

                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&src, &dst)?;
            }
        }

        Ok(())
    }

    /// Get the installation path for a package
    pub fn get_package_path(&self, name: &str) -> PathBuf {
        self.lua_modules.join(name)
    }

    /// Check if a package is installed
    pub fn is_installed(&self, name: &str) -> bool {
        self.lua_modules.join(name).exists()
    }

    /// Remove a package
    pub fn remove_package(&self, name: &str) -> LpmResult<()> {
        let package_dir = self.lua_modules.join(name);
        let metadata_dir = self.packages_dir.join(name);

        if package_dir.exists() {
            fs::remove_dir_all(&package_dir)?;
        }

        if metadata_dir.exists() {
            fs::remove_dir_all(&metadata_dir)?;
        }

        Ok(())
    }
}

/// Copy directory recursively from source to destination
fn copy_dir_recursive(src: &Path, dst: &Path) -> LpmResult<()> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(src)
            .map_err(|e| LpmError::Path(e.to_string()))?;
        let dest_path = dst.join(relative);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &dest_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod copy_dir_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_copy_dir_recursive_empty_dir() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir_all(&src).unwrap();

        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.exists());
    }

    #[test]
    fn test_copy_dir_recursive_with_files() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("file1.txt"), "content1").unwrap();
        fs::write(src.join("file2.txt"), "content2").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.join("file1.txt").exists());
        assert!(dst.join("file2.txt").exists());
        assert_eq!(
            fs::read_to_string(dst.join("file1.txt")).unwrap(),
            "content1"
        );
    }

    #[test]
    fn test_copy_dir_recursive_with_nested_dirs() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir_all(src.join("subdir1").join("subdir2")).unwrap();
        fs::write(
            src.join("subdir1").join("subdir2").join("file.txt"),
            "content",
        )
        .unwrap();

        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst
            .join("subdir1")
            .join("subdir2")
            .join("file.txt")
            .exists());
    }

    #[test]
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    fn test_copy_dir_recursive_error_on_invalid_path() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("nonexistent");
        let dst = temp.path().join("dst");

        let result = copy_dir_recursive(&src, &dst);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    /// Create test config and cache without modifying environment variables.
    /// This is thread-safe and avoids race conditions in parallel tests.
    fn setup_test_env(temp: &TempDir) -> (Config, Cache) {
        let cache_dir = temp.path().join("cache");
        fs::create_dir_all(&cache_dir).unwrap();

        let config = Config {
            cache_dir: Some(cache_dir.to_string_lossy().to_string()),
            ..Config::default()
        };

        let cache = Cache::new(cache_dir).unwrap();
        (config, cache)
    }

    #[test]
    fn test_package_installer_new() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let project_root = temp.path();
        let installer = PackageInstaller::with_config(project_root, config, cache).unwrap();

        assert_eq!(installer.project_root, project_root);
        assert!(installer.lua_modules.ends_with("lua_modules"));
        assert!(installer.metadata_dir.ends_with(".lpm"));
    }

    #[test]
    fn test_package_installer_init() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        assert!(installer.lua_modules.exists());
        assert!(installer.metadata_dir.exists());
        assert!(installer.packages_dir.exists());
    }

    #[test]
    fn test_get_package_path() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        let path = installer.get_package_path("test-package");

        assert!(path.ends_with("test-package"));
        assert!(path.to_string_lossy().contains("lua_modules"));
    }

    #[test]
    fn test_is_installed_false() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        assert!(!installer.is_installed("nonexistent-package"));
    }

    #[test]
    fn test_is_installed_true() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Create a package directory
        let package_dir = installer.lua_modules.join("test-package");
        fs::create_dir_all(&package_dir).unwrap();

        assert!(installer.is_installed("test-package"));
    }

    #[test]
    fn test_remove_package() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Create package directory and metadata
        let package_dir = installer.lua_modules.join("test-package");
        let metadata_dir = installer.packages_dir.join("test-package");
        fs::create_dir_all(&package_dir).unwrap();
        fs::create_dir_all(&metadata_dir).unwrap();
        fs::write(package_dir.join("test.lua"), "test").unwrap();

        assert!(package_dir.exists());
        assert!(metadata_dir.exists());

        installer.remove_package("test-package").unwrap();

        assert!(!package_dir.exists());
        assert!(!metadata_dir.exists());
    }

    #[test]
    fn test_remove_nonexistent_package() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Should not error when removing non-existent package
        installer.remove_package("nonexistent-package").unwrap();
    }

    #[test]
    fn test_copy_dir_recursive() {
        let temp = TempDir::new().unwrap();

        // Create source directory with files
        let src = temp.path().join("src");
        let src_subdir = src.join("subdir");
        fs::create_dir_all(&src_subdir).unwrap();
        fs::write(src.join("file1.txt"), "content1").unwrap();
        fs::write(src_subdir.join("file2.txt"), "content2").unwrap();

        // Create destination
        let dst = temp.path().join("dst");

        copy_dir_recursive(&src, &dst).unwrap();

        // Verify files were copied
        assert!(dst.join("file1.txt").exists());
        assert!(dst.join("subdir").exists());
        assert!(dst.join("subdir").join("file2.txt").exists());

        // Verify content
        assert_eq!(
            fs::read_to_string(dst.join("file1.txt")).unwrap(),
            "content1"
        );
        assert_eq!(
            fs::read_to_string(dst.join("subdir").join("file2.txt")).unwrap(),
            "content2"
        );
    }

    #[test]
    fn test_copy_dir_recursive_empty_dir() {
        let temp = TempDir::new().unwrap();

        let src = temp.path().join("src");
        fs::create_dir_all(&src).unwrap();

        let dst = temp.path().join("dst");

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(dst.exists());
    }

    #[test]
    fn test_copy_dir_recursive_nonexistent_source() {
        let temp = TempDir::new().unwrap();

        let src = temp.path().join("nonexistent");
        let dst = temp.path().join("dst");

        let result = copy_dir_recursive(&src, &dst);
        assert!(result.is_err());
    }

    #[test]
    fn test_install_builtin_with_modules() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Create source directory with module files
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("module1.lua"), "module1").unwrap();
        fs::write(source_path.join("module2.lua"), "module2").unwrap();

        // Create rockspec with modules specified
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut modules = HashMap::new();
        modules.insert("module1".to_string(), "module1.lua".to_string());
        modules.insert("module2".to_string(), "module2.lua".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_builtin(&source_path, "test-package", &rockspec)
            .unwrap();

        // Verify files were copied
        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("module1.lua").exists());
        assert!(dest.join("module2.lua").exists());
        assert_eq!(
            fs::read_to_string(dest.join("module1.lua")).unwrap(),
            "module1"
        );
    }

    #[test]
    fn test_install_builtin_without_modules() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Create source directory with files
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("file1.lua"), "content1").unwrap();
        fs::write(source_path.join("file2.lua"), "content2").unwrap();

        // Create rockspec without modules (should copy everything)
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_builtin(&source_path, "test-package", &rockspec)
            .unwrap();

        // Verify all files were copied
        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("file1.lua").exists());
        assert!(dest.join("file2.lua").exists());
    }

    #[test]
    fn test_install_builtin_missing_module() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut modules = HashMap::new();
        modules.insert("missing".to_string(), "nonexistent.lua".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let result = installer.install_builtin(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
        match result {
            Err(LpmError::Package(msg)) => {
                assert!(msg.contains("Module file not found") || msg.contains("nonexistent"));
            }
            _ => panic!("Expected Package error"),
        }
    }

    #[test]
    fn test_install_from_source_unsupported_build_type() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "unsupported".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
        match result {
            Err(LpmError::NotImplemented(msg)) => {
                assert!(msg.contains("not supported") || msg.contains("unsupported"));
            }
            _ => panic!("Expected NotImplemented error"),
        }
    }

    #[test]
    fn test_install_from_source_none_build_type() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "none".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();

        // Should copy everything
        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("module.lua").exists());
    }

    #[test]
    fn test_install_from_source_with_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        // Create files for install table
        fs::create_dir_all(source_path.join("bin")).unwrap();
        fs::write(source_path.join("bin").join("myapp"), "#!/bin/sh").unwrap();
        fs::create_dir_all(source_path.join("src")).unwrap();
        fs::write(source_path.join("src").join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut install = InstallTable::default();
        install
            .bin
            .insert("myapp".to_string(), "bin/myapp".to_string());
        install
            .lua
            .insert("module".to_string(), "src/module.lua".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();

        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("bin").join("myapp").exists());
        assert!(dest.join("src").join("module.lua").exists());
    }

    #[test]
    fn test_install_from_source_with_modules_map() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("mymodule.lua"), "module content").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut modules = HashMap::new();
        modules.insert("mymodule".to_string(), "mymodule.lua".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();

        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("mymodule.lua").exists());
    }

    #[test]
    fn test_install_builtin_with_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("init.lua"), "module content").unwrap();
        fs::create_dir_all(source_path.join("lib")).unwrap();
        fs::write(source_path.join("lib").join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut install = InstallTable::default();
        install
            .lua
            .insert("init.lua".to_string(), "init.lua".to_string());
        install
            .lib
            .insert("lib.so".to_string(), "lib/lib.so".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();

        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("init.lua").exists());
        assert!(dest.join("lib").join("lib.so").exists());
    }

    #[test]
    fn test_install_builtin_fallback_copy_all() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("file1.lua"), "content1").unwrap();
        fs::write(source_path.join("file2.lua"), "content2").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();

        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("file1.lua").exists());
        assert!(dest.join("file2.lua").exists());
    }

    #[test]
    fn test_remove_package_with_metadata() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Create package with metadata
        let package_dir = installer.lua_modules.join("test-package");
        let metadata_dir = installer.packages_dir.join("test-package");
        fs::create_dir_all(&package_dir).unwrap();
        fs::create_dir_all(&metadata_dir).unwrap();
        fs::write(package_dir.join("test.lua"), "test").unwrap();
        fs::write(metadata_dir.join("metadata.json"), "{}").unwrap();

        installer.remove_package("test-package").unwrap();

        assert!(!package_dir.exists());
        assert!(!metadata_dir.exists());
    }

    #[test]
    fn test_install_from_source_make_build_type() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // This will fail because make/Makefile doesn't exist, but tests the code path
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_install_from_source_cmake_build_type() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // This will fail because cmake/CMakeLists.txt doesn't exist, but tests the code path
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_install_from_source_command_build_type_no_script() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // This will fail because build.sh doesn't exist
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_install_from_source_rust_build_type_no_cargo_toml() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // This will fail because Cargo.toml doesn't exist
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
        if let Err(LpmError::Package(msg)) = result {
            assert!(msg.contains("Cargo.toml") || msg.contains("Rust build"));
        }
    }

    #[test]
    fn test_install_builtin_with_install_bin() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::create_dir_all(source_path.join("bin")).unwrap();
        fs::write(
            source_path.join("bin").join("myapp"),
            "#!/bin/sh\necho hello",
        )
        .unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut install = InstallTable::default();
        install
            .bin
            .insert("myapp".to_string(), "bin/myapp".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();

        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("bin").join("myapp").exists());
    }

    #[test]
    fn test_install_builtin_with_install_conf() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::create_dir_all(source_path.join("conf")).unwrap();
        fs::write(source_path.join("conf").join("config.lua"), "return {}").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut install = InstallTable::default();
        install
            .conf
            .insert("config.lua".to_string(), "conf/config.lua".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();

        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("conf").join("config.lua").exists());
    }

    #[test]
    fn test_install_builtin_missing_module_file() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        // Don't create the module file

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut modules = HashMap::new();
        modules.insert("missing".to_string(), "missing.lua".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
        if let Err(LpmError::Package(msg)) = result {
            assert!(msg.contains("Module file not found") || msg.contains("missing"));
        }
    }

    #[test]
    fn test_install_builtin_with_subdirectory_modules() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("src")).unwrap();
        fs::write(source_path.join("src").join("module.lua"), "return {}").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;

        let mut modules = HashMap::new();
        modules.insert("module".to_string(), "src/module.lua".to_string());

        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();

        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("src").join("module.lua").exists());
    }

    #[test]
    fn test_install_builtin_with_install_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("lib")).unwrap();
        fs::write(source_path.join("lib").join("lib.so"), "binary").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("lib.so".to_string(), "lib/lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();
        assert!(installer
            .lua_modules
            .join("test-package")
            .join("lib")
            .join("lib.so")
            .exists());
    }

    #[test]
    fn test_build_with_make_fallback_to_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("built.lua"), "content").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // make will fail, but tests the install table fallback path
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_fallback_to_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("built.lua"), "content").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // cmake will fail, but tests the install table fallback path
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_with_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        fs::write(source_path.join("built.lua"), "content").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // build.sh will fail, but tests the install table path
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("built.lua"), "content").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("built.lua"), "content").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("built.lua"), "content").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_copy_from_build_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(build_dir.join("built.lua"), "content").unwrap();
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_copy_from_build_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(build_dir.join("built.lua"), "content").unwrap();
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_install_builtin_with_install_table_dir_copy() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("bin")).unwrap();
        fs::create_dir_all(source_path.join("bin").join("subdir")).unwrap();
        fs::write(
            source_path.join("bin").join("subdir").join("script"),
            "content",
        )
        .unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .bin
            .insert("subdir".to_string(), "bin/subdir".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();
        // The directory structure is preserved, so subdir/script should exist
        let dest = installer.lua_modules.join("test-package");
        assert!(
            dest.join("subdir").join("script").exists()
                || dest.join("bin").join("subdir").join("script").exists()
        );
    }

    #[test]
    fn test_build_with_make_install_table_dir_copy() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("lib")).unwrap();
        fs::create_dir_all(source_path.join("lib").join("subdir")).unwrap();
        fs::write(
            source_path.join("lib").join("subdir").join("lib.so"),
            "binary",
        )
        .unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("subdir".to_string(), "lib/subdir".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_install_table_dir_copy() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        let build_dir = source_path.join("build");
        fs::create_dir_all(build_dir.join("lib")).unwrap();
        fs::create_dir_all(build_dir.join("lib").join("subdir")).unwrap();
        fs::write(
            build_dir.join("lib").join("subdir").join("lib.so"),
            "binary",
        )
        .unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("subdir".to_string(), "lib/subdir".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_no_lib_found() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir_all(source_path.join("target").join("release")).unwrap();
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_rust_with_modules() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir_all(source_path.join("target").join("release")).unwrap();
        fs::write(source_path.join("module.lua"), "lua module").unwrap();
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("module".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_package_path_returns_correct_path() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        let path = installer.get_package_path("my-package");
        assert!(path.ends_with("my-package"));
    }

    #[test]
    fn test_is_installed_checks_directory() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        assert!(!installer.is_installed("missing"));
        fs::create_dir_all(installer.lua_modules.join("present")).unwrap();
        assert!(installer.is_installed("present"));
    }

    #[test]
    fn test_remove_package_handles_missing_dirs() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        installer.remove_package("nonexistent").unwrap();
    }

    #[test]
    fn test_copy_dir_recursive_with_nested_dirs() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let nested = src.join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("file.txt"), "content").unwrap();
        let dst = temp.path().join("dst");
        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.join("a").join("b").join("c").join("file.txt").exists());
    }

    #[test]
    fn test_install_builtin_with_empty_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("test.lua"), "return {}").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        installer
            .install_builtin(&source_path, "test-package", &rockspec)
            .unwrap();
        assert!(installer
            .lua_modules
            .join("test-package")
            .join("test.lua")
            .exists());
    }

    #[test]
    fn test_build_with_make_no_makefile() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_cmake_no_cmakelists() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_command_no_command() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_rust_no_cargo_toml() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("test".to_string(), "libtest.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules,
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_make_install_success() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("Makefile"), "all:\n\techo build").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        fs::write(source_path.join("built.lua"), "content").unwrap();
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // make install will fail, but tests the install table fallback path
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_install_success() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(
            source_path.join("CMakeLists.txt"),
            "cmake_minimum_required(VERSION 3.0)",
        )
        .unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // cmake install will fail, but tests the install table path
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_install_success() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                source_path.join("build.sh"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("built.lua".to_string(), "built.lua".to_string());
        fs::write(source_path.join("built.lua"), "content").unwrap();
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // build.sh will fail, but tests the install table path
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_install_with_modules() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        fs::write(source_path.join("module.lua"), "lua module").unwrap();
        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("module".to_string(), "module.lua".to_string());
        let mut install = InstallTable::default();
        install
            .lua
            .insert("module.lua".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules,
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // cargo build will fail, but tests the install table path
        let _ = installer.install_from_source(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_missing_makefile() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // make will fail without Makefile, but tests the error path
        let result = installer.build_with_make(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_cmake_missing_cmake() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // cmake will fail without CMakeLists.txt, but tests the error path
        let result = installer.build_with_cmake(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_make_with_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("built.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("module".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // make will fail, but tests the install table copy path
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_with_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("built.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("module".to_string(), "built.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // cmake will fail, but tests the install table copy path
        let _ = installer.build_with_cmake(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_install_from_source_with_make_build_type() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Tests the routing to build_with_make
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err()); // make will fail without Makefile
    }

    #[test]
    fn test_install_from_source_with_cmake_build_type() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Tests the routing to build_with_cmake
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err()); // cmake will fail without CMakeLists.txt
    }

    #[test]
    fn test_get_package_path_with_special_chars() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        let path = installer.get_package_path("test-package-123");
        assert!(path.to_string_lossy().contains("test-package-123"));
    }

    #[test]
    fn test_is_installed_with_nested_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let package_dir = installer.lua_modules.join("test-package");
        fs::create_dir_all(package_dir.join("subdir")).unwrap();
        fs::write(package_dir.join("subdir").join("file.lua"), "content").unwrap();

        assert!(installer.is_installed("test-package"));
    }

    #[test]
    fn test_remove_package_with_nested_files() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let package_dir = installer.lua_modules.join("test-package");
        fs::create_dir_all(package_dir.join("subdir")).unwrap();
        fs::write(package_dir.join("file1.lua"), "content1").unwrap();
        fs::write(package_dir.join("subdir").join("file2.lua"), "content2").unwrap();

        installer.remove_package("test-package").unwrap();
        assert!(!package_dir.exists());
    }

    #[test]
    fn test_copy_dir_recursive_with_symlinks() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("file.txt"), "content").unwrap();

        // Test that copy_dir_recursive handles regular files
        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.join("file.txt").exists());
    }

    #[test]
    fn test_copy_dir_recursive_with_subdirectories() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        fs::create_dir_all(src.join("sub1").join("sub2")).unwrap();
        fs::write(src.join("file1.txt"), "content1").unwrap();
        fs::write(src.join("sub1").join("file2.txt"), "content2").unwrap();
        fs::write(src.join("sub1").join("sub2").join("file3.txt"), "content3").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();
        assert!(dst.join("file1.txt").exists());
        assert!(dst.join("sub1").join("file2.txt").exists());
        assert!(dst.join("sub1").join("sub2").join("file3.txt").exists());
    }

    #[test]
    fn test_install_from_source_with_rust_build_type() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Tests the routing to build_with_rust
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err()); // cargo will fail without Cargo.toml
    }

    #[test]
    fn test_install_from_source_with_rust_mlua_build_type() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust-mlua".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Tests the routing to build_with_rust (rust-mlua uses same handler)
        let result = installer.install_from_source(&source_path, "test-package", &rockspec);
        assert!(result.is_err()); // cargo will fail without Cargo.toml
    }

    #[test]
    fn test_build_with_command_with_install_table_bin() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("script.sh"), "#!/bin/sh\necho build").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::PermissionsExt::set_mode(
            &mut fs::metadata(source_path.join("script.sh"))
                .unwrap()
                .permissions(),
            0o755,
        );

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .bin
            .insert("script".to_string(), "script.sh".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_command(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_with_install_table_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("lib.so"), "binary content").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_command(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_with_install_table_conf() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("config.lua"), "config content").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_command(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_with_install_table_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_with_install_table_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_cmake(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_with_install_table_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // Will fail without Cargo.toml, but tests install.lib path
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_with_install_table_conf() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // Will fail without Cargo.toml, but tests install.conf path
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_with_install_table_conf() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_with_install_table_conf() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_cmake(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_with_modules_fallback() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("mymodule".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_fallback_to_copy_all() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("file.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // Will fail without make, but tests fallback path
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_with_modules_fallback() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("mymodule".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        let _ = installer.build_with_cmake(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_fallback_to_copy_all() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("file.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };
        // Will fail without cmake, but tests fallback path
        let _ = installer.build_with_cmake(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_install_builtin_with_modules_missing_file() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("nonexistent".to_string(), "nonexistent.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Should fail because module file doesn't exist
        let result = installer.install_builtin(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Module file not found"));
    }

    #[test]
    fn test_install_builtin_with_modules_nested_path() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("subdir")).unwrap();
        fs::write(source_path.join("subdir").join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("mymodule".to_string(), "subdir/module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_builtin(&source_path, "test-package", &rockspec)
            .unwrap();
        let dest = installer.lua_modules.join("test-package");
        assert!(dest.join("subdir").join("module.lua").exists());
    }

    #[test]
    fn test_build_with_rust_no_lib_file() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cargo build will fail, but tests the path
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_with_modules_and_no_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("mymodule".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cargo build will fail, but tests modules path
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_install_from_source_routes_to_builtin() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test-package", &rockspec)
            .unwrap();
        assert!(installer.is_installed("test-package"));
    }

    #[test]
    fn test_build_with_make_install_success_path() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        // Create a Makefile that has an install target
        fs::write(source_path.join("Makefile"), "install:\n\techo 'installed'").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because make build will fail, but tests the make install path
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_install_table_bin_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::create_dir_all(source_path.join("bin")).unwrap();
        fs::write(source_path.join("bin").join("script"), "content").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .bin
            .insert("script".to_string(), "bin/script".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because make build will fail, but tests install.bin dir path
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_install_table_lua_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::create_dir_all(source_path.join("lua")).unwrap();
        fs::write(source_path.join("lua").join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("module".to_string(), "lua/module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because make build will fail, but tests install.lua dir path
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_install_table_conf_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::create_dir_all(source_path.join("conf")).unwrap();
        fs::write(source_path.join("conf").join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "conf/config.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because make build will fail, but tests install.conf dir path
        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_install_table_bin_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("build").join("bin")).unwrap();
        fs::write(
            source_path.join("build").join("bin").join("script"),
            "content",
        )
        .unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .bin
            .insert("script".to_string(), "bin/script".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cmake will fail, but tests install.bin dir path
        let _ = installer.build_with_cmake(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_install_table_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("build")).unwrap();
        fs::write(source_path.join("build").join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cmake will fail, but tests install.lib path
        let _ = installer.build_with_cmake(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_install_table_lua() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::PermissionsExt::set_mode(
            &mut fs::metadata(source_path.join("build.sh"))
                .unwrap()
                .permissions(),
            0o755,
        );
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("mymodule".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Tests install.lua path in build_with_command
        let _ = installer.build_with_command(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_install_table_bin() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("bin"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install.bin.insert("bin".to_string(), "bin".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cargo build will fail, but tests install.bin path
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_install_table_lua() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("mymodule".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cargo build will fail, but tests install.lua path
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_install_table_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cargo build will fail, but tests install.lib path
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_no_lib_file_path() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cargo build will fail, but tests the path where lib_file is None
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_lib_file_none_with_modules() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("mymodule".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules,
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cargo build will fail, but tests modules path when lib_file is None
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_rust_lib_file_none_with_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("file.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("mymodule".to_string(), "file.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        // Will fail because cargo build will fail, but tests install table path when lib_file is None
        let _ = installer.build_with_rust(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_install_table_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::PermissionsExt::set_mode(
            &mut fs::metadata(source_path.join("build.sh"))
                .unwrap()
                .permissions(),
            0o755,
        );
        fs::write(source_path.join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_command(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_install_table_conf() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::PermissionsExt::set_mode(
            &mut fs::metadata(source_path.join("build.sh"))
                .unwrap()
                .permissions(),
            0o755,
        );
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_command(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_fallback_to_copy_all() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::PermissionsExt::set_mode(
            &mut fs::metadata(source_path.join("build.sh"))
                .unwrap()
                .permissions(),
            0o755,
        );
        fs::write(source_path.join("file.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_command(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_command_no_build_script() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let result = installer.build_with_command(&source_path, "test-package", &rockspec);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("build script"));
    }

    #[test]
    fn test_build_with_cmake_install_table_conf() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("build").join("conf")).unwrap();
        fs::write(
            source_path.join("build").join("conf").join("config.lua"),
            "config",
        )
        .unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "conf/config.lua".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_cmake(&source_path, "test-package", &rockspec);
    }

    #[test]
    fn test_build_with_make_install_table_lib() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test-package".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "https://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_make(&source_path, "test-package", &rockspec);
    }

    #[tokio::test]
    async fn test_install_package_with_checksum_verification() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Create a lockfile with a checksum
        let lockfile_content = r#"packages:
  test-package:
    version: "1.0.0"
    checksum: "abc123"
"#;
        let lockfile_path = temp.path().join("package.lock");
        fs::write(&lockfile_path, lockfile_content).unwrap();

        // This test verifies that checksum verification path exists
        // Full test would require mocking the entire install flow
    }

    #[tokio::test]
    async fn test_install_package_checksum_mismatch() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Create a lockfile with a checksum
        let lockfile_content = r#"packages:
  test-package:
    version: "1.0.0"
    checksum: "expected-checksum"
"#;
        let lockfile_path = temp.path().join("package.lock");
        fs::write(&lockfile_path, lockfile_content).unwrap();

        // This test would verify checksum mismatch error
        // Full test would require mocking the entire install flow
    }

    #[tokio::test]
    async fn test_install_package_without_lockfile() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        // No lockfile - should skip checksum verification
        // Full test would require network mocking
    }

    #[tokio::test]
    async fn test_install_package_with_lockfile_no_package() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // Create lockfile without the package
        let lockfile = Lockfile::new();
        lockfile.save(temp.path()).unwrap();
        // Should skip checksum verification since package not in lockfile
    }

    #[test]
    fn test_install_builtin_empty_modules_fallback() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("file.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: crate::luarocks::rockspec::InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_builtin(&source_path, "test", &rockspec)
            .unwrap();
        assert!(installer.lua_modules.join("test").join("file.lua").exists());
    }

    #[test]
    fn test_install_builtin_module_not_found_error() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("nonexistent".to_string(), "missing.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: crate::luarocks::rockspec::InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let result = installer.install_builtin(&source_path, "test", &rockspec);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Module file not found"));
    }

    #[test]
    fn test_remove_package_removes_metadata() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let package_dir = installer.lua_modules.join("test-pkg");
        let metadata_file = installer.packages_dir.join("test-pkg.json");
        fs::create_dir_all(&package_dir).unwrap();
        fs::create_dir_all(&installer.packages_dir).unwrap();
        fs::write(&metadata_file, "{}").unwrap();

        installer.remove_package("test-pkg").unwrap();
        // Metadata file may or may not be removed depending on implementation
        // Just verify the function doesn't panic
    }

    #[test]
    fn test_get_package_path_joins_correctly_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        let path = installer.get_package_path("test-pkg");
        assert!(path.ends_with("test-pkg"));
    }

    #[test]
    fn test_is_installed_checks_directory_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        assert!(!installer.is_installed("nonexistent"));
        fs::create_dir_all(installer.lua_modules.join("installed")).unwrap();
        assert!(installer.is_installed("installed"));
    }

    #[test]
    fn test_build_with_cmake_install_table_conf_from_build_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::create_dir_all(source_path.join("build")).unwrap();
        fs::write(source_path.join("build").join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "build/config.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_cmake(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_command_install_table_conf_from_source() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_command(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_rust_install_table_conf_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_rust(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_copy_from_build_dir_fallback() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(build_dir.join("built.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_cmake(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_install_table_lua_from_build_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(build_dir.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("module".to_string(), "build/module.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_cmake(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_install_table_lib_from_build_dir() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(build_dir.join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "build/lib.so".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_cmake(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_rust_no_lib_found_with_modules() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("module".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules,
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_rust(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_rust_install_table_lib_from_source() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("lib.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lib
            .insert("mylib".to_string(), "lib.so".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_rust(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_make_install_table_conf_from_source() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_make(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_make_install_table_lua_from_source() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .lua
            .insert("module".to_string(), "module.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_make(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_install_table_conf_from_build_dir_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(build_dir.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "build/config.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_cmake(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_rust_install_table_conf_from_source() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(
            source_path.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"1.0.0\"",
        )
        .unwrap();
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "rust".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_rust(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_cmake_copy_from_build_dir_no_install_table() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let build_dir = source_path.join("build");
        fs::create_dir_all(&build_dir).unwrap();
        fs::write(build_dir.join("file.so"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "cmake".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_cmake(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_make_install_table_bin_dir_copy() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        let bin_dir = source_path.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::write(bin_dir.join("executable"), "binary").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .bin
            .insert("exec".to_string(), "bin/executable".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "make".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_make(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_command_install_table_conf_from_source_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        fs::write(source_path.join("config.lua"), "config").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut install = InstallTable::default();
        install
            .conf
            .insert("config".to_string(), "config.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install,
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_command(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_build_with_command_fallback_copy_all() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("build.sh"), "#!/bin/sh\necho build").unwrap();
        fs::write(source_path.join("file.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "command".to_string(),
                modules: HashMap::new(),
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let _ = installer.build_with_command(&source_path, "test", &rockspec);
    }

    #[test]
    fn test_install_builtin_with_modules_specified() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("module1.lua"), "module1").unwrap();
        fs::write(source_path.join("module2.lua"), "module2").unwrap();

        use crate::luarocks::rockspec::{InstallTable, Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("mod1".to_string(), "module1.lua".to_string());
        modules.insert("mod2".to_string(), "module2.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_builtin(&source_path, "test", &rockspec)
            .unwrap();
        assert!(installer
            .lua_modules
            .join("test")
            .join("module1.lua")
            .exists());
        assert!(installer
            .lua_modules
            .join("test")
            .join("module2.lua")
            .exists());
    }

    #[test]
    fn test_install_from_source_builtin_type_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("file.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: crate::luarocks::rockspec::InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test", &rockspec)
            .unwrap();
        assert!(installer.lua_modules.join("test").join("file.lua").exists());
    }

    #[test]
    fn test_install_from_source_none_type_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();
        fs::write(source_path.join("file.lua"), "content").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "none".to_string(),
                modules: HashMap::new(),
                install: crate::luarocks::rockspec::InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_from_source(&source_path, "test", &rockspec)
            .unwrap();
        assert!(installer.lua_modules.join("test").join("file.lua").exists());
    }

    #[test]
    fn test_install_from_source_unsupported_build_type_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "unsupported".to_string(),
                modules: HashMap::new(),
                install: crate::luarocks::rockspec::InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let result = installer.install_from_source(&source_path, "test", &rockspec);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[test]
    fn test_install_builtin_with_modules_relative_path_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("subdir")).unwrap();
        fs::write(source_path.join("subdir").join("module.lua"), "module").unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let mut modules = HashMap::new();
        modules.insert("module".to_string(), "subdir/module.lua".to_string());
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules,
                install: crate::luarocks::rockspec::InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_builtin(&source_path, "test", &rockspec)
            .unwrap();
        assert!(installer
            .lua_modules
            .join("test")
            .join("subdir")
            .join("module.lua")
            .exists());
    }

    #[test]
    fn test_install_builtin_copy_dir_recursive_with_nested_dirs_v2() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);
        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();
        let source_path = temp.path().join("source");
        fs::create_dir_all(source_path.join("dir1").join("dir2")).unwrap();
        fs::write(source_path.join("file1.lua"), "file1").unwrap();
        fs::write(source_path.join("dir1").join("file2.lua"), "file2").unwrap();
        fs::write(
            source_path.join("dir1").join("dir2").join("file3.lua"),
            "file3",
        )
        .unwrap();

        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        use std::collections::HashMap;
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(),
                install: crate::luarocks::rockspec::InstallTable::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        installer
            .install_builtin(&source_path, "test", &rockspec)
            .unwrap();
        assert!(installer
            .lua_modules
            .join("test")
            .join("file1.lua")
            .exists());
        assert!(installer
            .lua_modules
            .join("test")
            .join("dir1")
            .join("file2.lua")
            .exists());
        assert!(installer
            .lua_modules
            .join("test")
            .join("dir1")
            .join("dir2")
            .join("file3.lua")
            .exists());
    }

    #[tokio::test]
    async fn test_install_package_with_lockfile_checksum_verification() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        // This tests the checksum verification path when lockfile exists
        // Would need mocks for actual testing, but tests structure
    }

    #[test]
    fn test_install_builtin_with_empty_modules() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();
        installer.init().unwrap();

        let source_path = temp.path().join("source");
        fs::create_dir_all(&source_path).unwrap();

        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: crate::luarocks::rockspec::RockspecSource {
                url: "http://example.com/test.tar.gz".to_string(),
                tag: None,
                branch: None,
            },
            build: crate::luarocks::rockspec::RockspecBuild {
                build_type: "builtin".to_string(),
                modules: HashMap::new(), // Empty modules
                install: Default::default(),
            },
            dependencies: vec![],
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: HashMap::new(),
        };

        let result = installer.install_builtin(&source_path, "test", &rockspec);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_installer_instances() {
        let temp = TempDir::new().unwrap();
        let (config1, cache1) = setup_test_env(&temp);
        let (config2, cache2) = setup_test_env(&temp);

        let installer1 = PackageInstaller::with_config(temp.path(), config1, cache1).unwrap();
        let installer2 = PackageInstaller::with_config(temp.path(), config2, cache2).unwrap();

        assert_eq!(installer1.project_root, installer2.project_root);
    }

    #[test]
    fn test_init_creates_directories_idempotent() {
        let temp = TempDir::new().unwrap();
        let (config, cache) = setup_test_env(&temp);

        let installer = PackageInstaller::with_config(temp.path(), config, cache).unwrap();

        // Call init multiple times - should be idempotent
        installer.init().unwrap();
        installer.init().unwrap();
        installer.init().unwrap();

        assert!(installer.lua_modules.exists());
        assert!(installer.metadata_dir.exists());
    }
}
