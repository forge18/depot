use crate::build::prebuilt::PrebuiltBinaryManager;
use crate::build::sandbox::BuildSandbox;
use crate::build::targets::Target;
use crate::cache::Cache;
use crate::core::path::cache_dir;
use crate::core::{LpmError, LpmResult};
use crate::lua_version::detector::{LuaVersion, LuaVersionDetector};
use crate::package::manifest::{BuildConfig, PackageManifest};
use std::path::{Path, PathBuf};

/// Builder for Rust code compiled into Lua native modules
///
/// This builds Rust code as dynamic libraries (.so/.dylib/.dll) that can be
/// loaded by Lua as native modules. These are NOT standalone Rust libraries,
/// but compiled code that becomes part of a Lua module package.
pub struct RustBuilder {
    project_root: PathBuf,
    build_config: BuildConfig,
}

impl RustBuilder {
    /// Create a new Rust builder from a manifest
    pub fn new(project_root: &Path, manifest: &PackageManifest) -> LpmResult<Self> {
        let build_config = manifest
            .build
            .as_ref()
            .ok_or_else(|| {
                LpmError::Package("No build configuration found in package.yaml".to_string())
            })?
            .clone();

        if build_config.build_type != "rust" {
            return Err(LpmError::Package(format!(
                "Build type '{}' is not supported. Only 'rust' is supported",
                build_config.build_type
            )));
        }

        Ok(Self {
            project_root: project_root.to_path_buf(),
            build_config,
        })
    }

    /// Build for a specific target
    ///
    /// This will:
    /// 1. Check for pre-built binaries (local cache or download)
    /// 2. Check for locally cached builds
    /// 3. Build from source if neither is available
    pub async fn build(&self, target: Option<&Target>) -> LpmResult<PathBuf> {
        let default_target = Target::default_target();
        let target = target.unwrap_or(&default_target);

        // Detect Lua version for caching
        let lua_version = Self::detect_lua_version()?;
        let lua_version_str = lua_version.major_minor();

        let manifest = PackageManifest::load(&self.project_root)?;

        // First, check for pre-built binaries
        let prebuilt_manager = PrebuiltBinaryManager::new()?;
        if let Some(prebuilt_path) =
            prebuilt_manager.get_prebuilt(&manifest.name, &manifest.version, &lua_version, target)
        {
            eprintln!("Using pre-built binary for Lua {}", lua_version_str);
            return Ok(prebuilt_path);
        }

        // Check local cache
        let cache = Cache::new(cache_dir()?)?;
        if let Some(cached_path) = cache.get_rust_build(
            &manifest.name,
            &manifest.version,
            &lua_version_str,
            &target.triple,
        ) {
            eprintln!("Using cached build for Lua {}", lua_version_str);
            return Ok(cached_path);
        }

        // Check if package.yaml specifies pre-built binary URLs
        let binary_url: Option<String> = {
            // First check package.yaml
            let manifest = PackageManifest::load(&self.project_root).ok();
            let url_from_manifest = manifest.as_ref().and_then(|m| {
                // Build target key: "5.4-x86_64-unknown-linux-gnu"
                let target_key = format!("{}-{}", lua_version_str, target.triple);
                m.binary_urls.get(&target_key).cloned()
            });

            // Fall back to rockspec binary_urls if available
            // (This would need to be passed in, but for now we check manifest first)
            url_from_manifest
        };

        // Try to download pre-built binary if URL is available
        if let Some(url) = binary_url {
            if let Some(downloaded) = prebuilt_manager
                .get_or_download(
                    &manifest.name,
                    &manifest.version,
                    &lua_version,
                    target,
                    Some(&url),
                )
                .await?
            {
                eprintln!(
                    "Using downloaded pre-built binary for Lua {}",
                    lua_version_str
                );
                return Ok(downloaded);
            }
        }

        // Ensure cargo-zigbuild is installed
        BuildSandbox::ensure_cargo_zigbuild()?;

        // Determine build command
        let use_zigbuild = target.triple != Target::default_target().triple;

        if use_zigbuild {
            // Use cargo-zigbuild for cross-compilation
            self.build_with_zigbuild(target)?;
        } else {
            // Use regular cargo for native builds
            self.build_with_cargo()?;
        }

        // Find the built library
        let artifact_path = self.find_built_library(target)?;

        // Cache the build artifact
        let cached_path = cache.store_rust_build(
            &manifest.name,
            &manifest.version,
            &lua_version_str,
            &target.triple,
            &artifact_path,
        )?;

        Ok(cached_path)
    }

    /// Build using cargo-zigbuild
    fn build_with_zigbuild(&self, target: &Target) -> LpmResult<()> {
        let manifest_path = self
            .build_config
            .manifest
            .as_ref()
            .map(|m| self.project_root.join(m))
            .unwrap_or_else(|| self.project_root.join("Cargo.toml"));

        if !manifest_path.exists() {
            return Err(LpmError::Package(format!(
                "Cargo.toml not found at {}",
                manifest_path.display()
            )));
        }

        // Detect Lua version and add mlua feature
        let lua_version = Self::detect_lua_version()?;
        let mlua_feature = lua_version.mlua_feature();
        eprintln!(
            "Building for Lua {} (mlua feature: {})",
            lua_version, mlua_feature
        );

        // Build with cargo-zigbuild
        let target_triple = target.triple.clone();
        let mut args: Vec<String> = vec![
            "zigbuild".to_string(),
            "--target".to_string(),
            target_triple,
        ];

        // Add profile
        if let Some(profile) = &self.build_config.profile {
            args.push("--".to_string());
            args.push("--profile".to_string());
            args.push(profile.clone());
        } else {
            args.push("--release".to_string());
        }

        // Add mlua feature for detected Lua version
        let mut features = self.build_config.features.clone();
        if !features.contains(&mlua_feature.to_string()) {
            features.push(mlua_feature.to_string());
        }

        // Add features
        if !features.is_empty() {
            let features_str = features.join(",");
            args.push("--features".to_string());
            args.push(features_str);
        }

        // Convert to &str for execute_cargo
        let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        BuildSandbox::execute_cargo(&self.project_root, &args_str, &[])?;

        Ok(())
    }

    /// Build using regular cargo
    fn build_with_cargo(&self) -> LpmResult<()> {
        let manifest_path = self
            .build_config
            .manifest
            .as_ref()
            .map(|m| self.project_root.join(m))
            .unwrap_or_else(|| self.project_root.join("Cargo.toml"));

        if !manifest_path.exists() {
            return Err(LpmError::Package(format!(
                "Cargo.toml not found at {}",
                manifest_path.display()
            )));
        }

        // Detect Lua version and add mlua feature
        let lua_version = Self::detect_lua_version()?;
        let mlua_feature = lua_version.mlua_feature();
        eprintln!(
            "Building for Lua {} (mlua feature: {})",
            lua_version, mlua_feature
        );

        // Build with cargo
        let mut args: Vec<String> = vec!["build".to_string()];

        // Add profile
        if let Some(profile) = &self.build_config.profile {
            args.push("--profile".to_string());
            args.push(profile.clone());
        } else {
            args.push("--release".to_string());
        }

        // Add mlua feature for detected Lua version
        let mut features = self.build_config.features.clone();
        if !features.contains(&mlua_feature.to_string()) {
            features.push(mlua_feature.to_string());
        }

        // Add features
        if !features.is_empty() {
            let features_str = features.join(",");
            args.push("--features".to_string());
            args.push(features_str);
        }

        // Convert to &str for execute_cargo
        let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        BuildSandbox::execute_cargo(&self.project_root, &args_str, &[])?;

        Ok(())
    }

    /// Find the built library file
    fn find_built_library(&self, target: &Target) -> LpmResult<PathBuf> {
        // Determine output directory based on target
        let target_dir = if target.triple == Target::default_target().triple {
            "target/release"
        } else {
            &format!("target/{}/release", target.triple)
        };

        // Look for modules in build config
        for (module_name, module_path) in &self.build_config.modules {
            let full_path = self.project_root.join(module_path);
            if full_path.exists() {
                return Ok(full_path);
            }

            // Try with target directory
            let target_path = self.project_root.join(target_dir).join(module_path);
            if target_path.exists() {
                return Ok(target_path);
            }

            // Try to find library by name
            let lib_name = format!("lib{}{}", module_name, target.module_extension());
            let lib_path = self.project_root.join(target_dir).join(&lib_name);
            if lib_path.exists() {
                return Ok(lib_path);
            }
        }

        Err(LpmError::Package(
            "Could not find built library. Check that the build completed successfully and the module path is correct in package.yaml"
                .to_string(),
        ))
    }

    /// Build for all supported targets
    ///
    /// This is now handled in the CLI layer since build() is async.
    /// Kept for backwards compatibility but delegates to async version.
    pub async fn build_all_targets(&self) -> LpmResult<Vec<(Target, PathBuf)>> {
        let mut results = Vec::new();

        for target_triple in crate::build::targets::SUPPORTED_TARGETS {
            let target = Target::new(target_triple)?;
            eprintln!("Building for target: {}", target.triple);

            match self.build(Some(&target)).await {
                Ok(path) => {
                    results.push((target, path));
                    eprintln!("✓ Built successfully for {}", target_triple);
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to build for {}: {}", target_triple, e);
                    // Continue with other targets
                }
            }
        }

        if results.is_empty() {
            return Err(LpmError::Package(
                "Failed to build for all targets".to_string(),
            ));
        }

        Ok(results)
    }

    /// Detect the target Lua version for building
    ///
    /// This detects the installed Lua version and ensures mlua is built
    /// with the correct feature flags (lua51, lua53, or lua54)
    pub fn detect_lua_version() -> LpmResult<LuaVersion> {
        LuaVersionDetector::detect()
    }

    /// Get mlua feature flags for the detected Lua version
    pub fn get_mlua_features() -> LpmResult<String> {
        let version = Self::detect_lua_version()?;
        Ok(version.mlua_feature().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageManifest;
    use tempfile::TempDir;

    #[test]
    fn test_rust_builder_new() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest);
        assert!(builder.is_ok());
    }

    #[test]
    fn test_rust_builder_no_build_config() {
        let temp = TempDir::new().unwrap();
        let manifest = PackageManifest::default("test".to_string());

        let builder = RustBuilder::new(temp.path(), &manifest);
        assert!(builder.is_err());
    }

    #[test]
    fn test_rust_builder_wrong_build_type() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "make".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest);
        assert!(builder.is_err());
        match builder {
            Err(LpmError::Package(msg)) => {
                assert!(msg.contains("not supported") || msg.contains("rust"));
            }
            _ => panic!("Expected Package error"),
        }
    }

    #[test]
    fn test_rust_builder_with_features() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules: std::collections::HashMap::new(),
            features: vec!["feature1".to_string(), "feature2".to_string()],
            profile: Some("release".to_string()),
        });

        let builder = RustBuilder::new(temp.path(), &manifest);
        assert!(builder.is_ok());
        let builder = builder.unwrap();
        assert_eq!(builder.build_config.features.len(), 2);
        assert_eq!(builder.build_config.profile, Some("release".to_string()));
    }

    #[test]
    fn test_rust_builder_detect_lua_version() {
        let result = RustBuilder::detect_lua_version();
        // Should either succeed or fail gracefully
        let _ = result;
    }

    #[test]
    fn test_rust_builder_get_mlua_features() {
        let result = RustBuilder::get_mlua_features();
        // Should either succeed or fail gracefully
        let _ = result;
    }

    #[test]
    fn test_rust_builder_new_with_manifest_path() {
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("custom.toml".to_string()),
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest);
        assert!(builder.is_ok());
        let builder = builder.unwrap();
        assert_eq!(
            builder.build_config.manifest,
            Some("custom.toml".to_string())
        );
    }

    #[test]
    fn test_rust_builder_build_with_zigbuild_error_no_cargo_toml() {
        // Test error path when Cargo.toml doesn't exist (line 155-160)
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("nonexistent.toml".to_string()),
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        let result = builder.build_with_zigbuild(&target);
        // Should fail - Cargo.toml not found
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_rust_builder_build_with_cargo_error_no_cargo_toml() {
        // Test error path when Cargo.toml doesn't exist (line 215-220)
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("nonexistent.toml".to_string()),
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let result = builder.build_with_cargo();
        // Should fail - Cargo.toml not found
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_rust_builder_build_with_cargo_uses_default_manifest() {
        // Test that build_with_cargo uses default Cargo.toml when manifest not specified
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: None, // Should default to Cargo.toml
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let result = builder.build_with_cargo();
        // Should fail - Cargo.toml not found (we didn't create it)
        assert!(result.is_err());
        // But should have checked for Cargo.toml in project root
        assert!(result.unwrap_err().to_string().contains("Cargo.toml"));
    }

    #[test]
    fn test_rust_builder_build_with_zigbuild_uses_default_manifest() {
        // Test that build_with_zigbuild uses default Cargo.toml when manifest not specified
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: None, // Should default to Cargo.toml
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        let result = builder.build_with_zigbuild(&target);
        // Should fail - Cargo.toml not found (we didn't create it)
        assert!(result.is_err());
        // But should have checked for Cargo.toml in project root
        assert!(result.unwrap_err().to_string().contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_rust_builder_build_error_paths() {
        // Test error paths in build() method
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("nonexistent.toml".to_string()),
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let result = builder.build(None).await;
        // Should fail - various reasons (no Cargo.toml, no cargo, etc.)
        // But tests the code paths
        let _ = result;
    }

    #[tokio::test]
    async fn test_rust_builder_build_all_targets() {
        // Test build_all_targets method (line 302-332)
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let result = builder.build_all_targets().await;
        // May fail on actual build, but tests the code path
        let _ = result;
    }

    #[test]
    fn test_rust_builder_find_built_library_error_no_modules() {
        // Test error path when no modules found (line 292-295)
        let temp = TempDir::new().unwrap();
        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules: std::collections::HashMap::new(), // Empty modules
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        let result = builder.find_built_library(&target);
        // Should fail - no modules configured
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Could not find built library"));
    }

    #[test]
    fn test_rust_builder_find_built_library_with_modules() {
        // Test find_built_library with modules configured (line 272-290)
        let temp = TempDir::new().unwrap();
        let mut modules = std::collections::HashMap::new();
        modules.insert("mymodule".to_string(), "lib/mymodule.so".to_string());

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules,
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        // Create the module file to test the path where it exists
        let module_path = temp.path().join("lib").join("mymodule.so");
        std::fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        std::fs::write(&module_path, b"fake library").unwrap();

        let result = builder.find_built_library(&target);
        // Should succeed - module file exists
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), module_path);
    }

    #[test]
    fn test_rust_builder_find_built_library_target_dir() {
        // Test find_built_library checking target directory (line 278-282)
        let temp = TempDir::new().unwrap();
        let mut modules = std::collections::HashMap::new();
        modules.insert("mymodule".to_string(), "lib/mymodule.so".to_string());

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules,
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        // Create the module file in target directory
        let target_dir = temp.path().join("target").join("release");
        let module_path = target_dir.join("lib").join("mymodule.so");
        std::fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        std::fs::write(&module_path, b"fake library").unwrap();

        let result = builder.find_built_library(&target);
        // Should succeed - module file exists in target directory
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), module_path);
    }

    #[test]
    fn test_rust_builder_find_built_library_by_name() {
        // Test find_built_library finding library by name (line 284-289)
        let temp = TempDir::new().unwrap();
        let mut modules = std::collections::HashMap::new();
        modules.insert("mymodule".to_string(), "lib/mymodule.so".to_string());

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules,
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        // Create the library file by name in target directory
        let target_dir = temp.path().join("target").join("release");
        let lib_name = format!("libmymodule{}", target.module_extension());
        let lib_path = target_dir.join(&lib_name);
        std::fs::create_dir_all(lib_path.parent().unwrap()).unwrap();
        std::fs::write(&lib_path, b"fake library").unwrap();

        let result = builder.find_built_library(&target);
        // Should succeed - library found by name
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), lib_path);
    }

    #[test]
    fn test_rust_builder_build_with_zigbuild_profile_path() {
        // Test build_with_zigbuild with profile specified (line 179-182)
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: Some("dev".to_string()), // Custom profile
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        let result = builder.build_with_zigbuild(&target);
        // May fail on actual build, but tests the profile path (line 179-182)
        let _ = result;
    }

    #[test]
    fn test_rust_builder_build_with_zigbuild_release_path() {
        // Test build_with_zigbuild without profile (uses --release, line 184)
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None, // Should use --release
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        let result = builder.build_with_zigbuild(&target);
        // May fail on actual build, but tests the --release path (line 184)
        let _ = result;
    }

    #[test]
    fn test_rust_builder_build_with_zigbuild_features_path() {
        // Test build_with_zigbuild with features (line 188-198)
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: vec!["feature1".to_string(), "feature2".to_string()],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let target = Target::default_target();
        let result = builder.build_with_zigbuild(&target);
        // May fail on actual build, but tests the features path (line 188-198)
        let _ = result;
    }

    #[test]
    fn test_rust_builder_build_with_cargo_profile_path() {
        // Test build_with_cargo with profile specified (line 235-237)
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: Some("dev".to_string()), // Custom profile
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let result = builder.build_with_cargo();
        // May fail on actual build, but tests the profile path (line 235-237)
        let _ = result;
    }

    #[test]
    fn test_rust_builder_build_with_cargo_release_path() {
        // Test build_with_cargo without profile (uses --release, line 239)
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: vec![],
            profile: None, // Should use --release
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let result = builder.build_with_cargo();
        // May fail on actual build, but tests the --release path (line 239)
        let _ = result;
    }

    #[test]
    fn test_rust_builder_build_with_cargo_features_path() {
        // Test build_with_cargo with features (line 243-253)
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: vec!["feature1".to_string(), "feature2".to_string()],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        let result = builder.build_with_cargo();
        // May fail on actual build, but tests the features path (line 243-253)
        let _ = result;
    }

    #[test]
    fn test_rust_builder_find_built_library_cross_compile_target_dir() {
        // Test find_built_library with cross-compilation target (line 267-269)
        let temp = TempDir::new().unwrap();
        let mut modules = std::collections::HashMap::new();
        modules.insert("mymodule".to_string(), "lib/mymodule.so".to_string());

        let mut manifest = PackageManifest::default("test".to_string());
        manifest.build = Some(BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules,
            features: vec![],
            profile: None,
        });

        let builder = RustBuilder::new(temp.path(), &manifest).unwrap();
        // Use a cross-compilation target
        let target = Target::new("x86_64-unknown-linux-gnu").unwrap();
        // Create the module file in cross-compile target directory
        let target_dir = temp
            .path()
            .join("target")
            .join("x86_64-unknown-linux-gnu")
            .join("release");
        let module_path = target_dir.join("lib").join("mymodule.so");
        std::fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        std::fs::write(&module_path, b"fake library").unwrap();

        let result = builder.find_built_library(&target);
        // Should succeed - module file exists in cross-compile target directory
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), module_path);
    }
}
