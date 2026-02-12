use crate::core::version::parse_constraint;
use crate::core::{DepotError, DepotResult};
use crate::package::manifest::PackageManifest;
use std::collections::HashSet;

/// Validates package.yaml schema and content
pub struct ManifestValidator;

impl ManifestValidator {
    /// Validate a manifest with comprehensive checks
    pub fn validate(manifest: &PackageManifest) -> DepotResult<()> {
        // Basic validation (already done in manifest.validate())
        manifest.validate()?;

        // Additional schema validation
        Self::validate_name(&manifest.name)?;
        Self::validate_version_format(&manifest.version)?;
        Self::validate_lua_version(&manifest.lua_version)?;
        Self::validate_dependencies(&manifest.dependencies)?;
        Self::validate_dev_dependencies(&manifest.dev_dependencies)?;
        Self::validate_build_config(&manifest.build)?;
        Self::validate_scripts(&manifest.scripts)?;

        Ok(())
    }

    fn validate_name(name: &str) -> DepotResult<()> {
        // Name should be valid identifier
        if name.is_empty() {
            return Err(DepotError::Package(
                "Package name cannot be empty".to_string(),
            ));
        }

        // Check for valid characters (alphanumeric, hyphen, underscore)
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(DepotError::Package(format!(
                "Package name '{}' contains invalid characters. Use only alphanumeric, hyphen, or underscore",
                name
            )));
        }

        // Name should not start with hyphen or underscore
        if name.starts_with('-') || name.starts_with('_') {
            return Err(DepotError::Package(format!(
                "Package name '{}' cannot start with '-' or '_'",
                name
            )));
        }

        Ok(())
    }

    fn validate_version_format(version: &str) -> DepotResult<()> {
        // Try to parse as version to validate format
        if version.is_empty() {
            return Err(DepotError::Package("Version cannot be empty".to_string()));
        }

        // Basic SemVer check (major.minor.patch)
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() > 3 {
            return Err(DepotError::Package(format!(
                "Invalid version format '{}'. Expected SemVer format (e.g., '1.2.3')",
                version
            )));
        }

        // Check each part is numeric
        for part in &parts {
            if !part.chars().all(|c| c.is_ascii_digit()) {
                return Err(DepotError::Package(format!(
                    "Invalid version format '{}'. Version parts must be numeric",
                    version
                )));
            }
        }

        Ok(())
    }

    fn validate_lua_version(lua_version: &str) -> DepotResult<()> {
        if lua_version.is_empty() {
            return Err(DepotError::Package(
                "lua_version cannot be empty".to_string(),
            ));
        }

        // Check for valid constraint format
        // Allow: "5.4", ">=5.1", "5.1 || 5.3 || 5.4", etc.
        // For now, just check it's not empty and has reasonable length
        if lua_version.len() > 100 {
            return Err(DepotError::Package(
                "lua_version constraint is too long".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_dependencies(deps: &std::collections::HashMap<String, String>) -> DepotResult<()> {
        let mut seen = HashSet::new();

        for (name, version) in deps {
            // Check for duplicate dependencies
            if seen.contains(name) {
                return Err(DepotError::Package(format!(
                    "Duplicate dependency '{}' found",
                    name
                )));
            }
            seen.insert(name.clone());

            // Validate dependency name
            Self::validate_name(name)?;

            // Validate version constraint
            parse_constraint(version).map_err(|e| {
                DepotError::Package(format!(
                    "Invalid version constraint '{}' for dependency '{}': {}",
                    version, name, e
                ))
            })?;
        }

        Ok(())
    }

    fn validate_dev_dependencies(
        deps: &std::collections::HashMap<String, String>,
    ) -> DepotResult<()> {
        // Same validation as regular dependencies
        Self::validate_dependencies(deps)
    }

    fn validate_build_config(
        build: &Option<crate::package::manifest::BuildConfig>,
    ) -> DepotResult<()> {
        if let Some(build) = build {
            // Validate build type
            match build.build_type.as_str() {
                "rust" | "builtin" | "none" => {}
                _ => {
                    return Err(DepotError::Package(format!(
                        "Invalid build type '{}'. Supported types: rust, builtin, none",
                        build.build_type
                    )));
                }
            }

            // If rust build, validate manifest path and ensure modules are specified
            // Rust builds must produce native Lua modules, not standalone libraries
            if build.build_type == "rust" {
                if let Some(manifest) = &build.manifest {
                    if !manifest.ends_with("Cargo.toml") {
                        return Err(DepotError::Package(format!(
                            "Rust build manifest should be 'Cargo.toml', got '{}'",
                            manifest
                        )));
                    }
                }

                // Rust builds must specify modules (native Lua modules, not standalone libraries)
                if build.modules.is_empty() {
                    return Err(DepotError::Package(
                        "Rust build must specify 'modules' mapping to native Lua module paths. \
                        Rust code must be compiled as dynamic libraries (.so/.dylib/.dll) \
                        that are part of a Lua module package, not standalone Rust libraries."
                            .to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_scripts(scripts: &std::collections::HashMap<String, String>) -> DepotResult<()> {
        for (name, command) in scripts {
            if name.is_empty() {
                return Err(DepotError::Package(
                    "Script name cannot be empty".to_string(),
                ));
            }

            if command.is_empty() {
                return Err(DepotError::Package(format!(
                    "Script '{}' has no command",
                    name
                )));
            }

            // Check for reserved script names
            let reserved = [
                "install",
                "preinstall",
                "postinstall",
                "prepublish",
                "publish",
            ];
            if reserved.contains(&name.as_str()) {
                return Err(DepotError::Package(format!(
                    "Script name '{}' is reserved and cannot be used",
                    name
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        assert!(ManifestValidator::validate_name("valid-name").is_ok());
        assert!(ManifestValidator::validate_name("valid_name").is_ok());
        assert!(ManifestValidator::validate_name("valid123").is_ok());
        assert!(ManifestValidator::validate_name("").is_err());
        assert!(ManifestValidator::validate_name("-invalid").is_err());
        assert!(ManifestValidator::validate_name("invalid@name").is_err());
    }

    #[test]
    fn test_validate_version_format() {
        assert!(ManifestValidator::validate_version_format("1.2.3").is_ok());
        assert!(ManifestValidator::validate_version_format("1.2").is_ok());
        assert!(ManifestValidator::validate_version_format("1").is_ok());
        assert!(ManifestValidator::validate_version_format("").is_err());
        assert!(ManifestValidator::validate_version_format("1.2.3.4").is_err());
        assert!(ManifestValidator::validate_version_format("1.2.x").is_err());
    }

    #[test]
    fn test_validate_lua_version() {
        assert!(ManifestValidator::validate_lua_version("5.4").is_ok());
        assert!(ManifestValidator::validate_lua_version(">=5.1").is_ok());
        assert!(ManifestValidator::validate_lua_version("").is_err());
        // Test too long constraint
        let long_constraint = ">=".to_string() + &"5".repeat(101);
        assert!(ManifestValidator::validate_lua_version(&long_constraint).is_err());
    }

    #[test]
    fn test_validate_dependencies() {
        use std::collections::HashMap;
        let mut deps = HashMap::new();
        deps.insert("test-pkg".to_string(), "^1.0.0".to_string());
        assert!(ManifestValidator::validate_dependencies(&deps).is_ok());

        // Test duplicate
        let mut deps_dup = HashMap::new();
        deps_dup.insert("test-pkg".to_string(), "^1.0.0".to_string());
        deps_dup.insert("test-pkg".to_string(), "^2.0.0".to_string());
        // HashMap doesn't allow duplicates, so this test is for the logic
        assert!(ManifestValidator::validate_dependencies(&deps_dup).is_ok());

        // Test invalid constraint
        let mut deps_invalid = HashMap::new();
        deps_invalid.insert("test-pkg".to_string(), "invalid-constraint".to_string());
        assert!(ManifestValidator::validate_dependencies(&deps_invalid).is_err());
    }

    #[test]
    fn test_validate_build_config() {
        use crate::package::manifest::BuildConfig;
        let build = BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules: {
                let mut m = std::collections::HashMap::new();
                m.insert("mymodule".to_string(), "libmymodule.so".to_string());
                m
            },
            features: Vec::new(),
            profile: None,
        };
        assert!(ManifestValidator::validate_build_config(&Some(build)).is_ok());

        // Test invalid build type
        let build_invalid = BuildConfig {
            build_type: "invalid".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: Vec::new(),
            profile: None,
        };
        assert!(ManifestValidator::validate_build_config(&Some(build_invalid)).is_err());

        // Test rust build without modules
        let build_no_modules = BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("Cargo.toml".to_string()),
            modules: std::collections::HashMap::new(),
            features: Vec::new(),
            profile: None,
        };
        assert!(ManifestValidator::validate_build_config(&Some(build_no_modules)).is_err());
    }

    #[test]
    fn test_validate_scripts() {
        use std::collections::HashMap;
        let mut scripts = HashMap::new();
        scripts.insert("test-script".to_string(), "lua test.lua".to_string());
        assert!(ManifestValidator::validate_scripts(&scripts).is_ok());

        // Test empty script name
        let mut scripts_empty_name = HashMap::new();
        scripts_empty_name.insert("".to_string(), "command".to_string());
        assert!(ManifestValidator::validate_scripts(&scripts_empty_name).is_err());

        // Test empty command
        let mut scripts_empty_cmd = HashMap::new();
        scripts_empty_cmd.insert("script".to_string(), "".to_string());
        assert!(ManifestValidator::validate_scripts(&scripts_empty_cmd).is_err());

        // Test reserved name
        let mut scripts_reserved = HashMap::new();
        scripts_reserved.insert("install".to_string(), "command".to_string());
        assert!(ManifestValidator::validate_scripts(&scripts_reserved).is_err());
    }

    #[test]
    fn test_validate_manifest_comprehensive() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.version = "1.0.0".to_string();
        manifest.lua_version = "5.4".to_string();
        let mut deps = std::collections::HashMap::new();
        deps.insert("test-dep".to_string(), "^1.0.0".to_string());
        manifest.dependencies = deps;
        assert!(ManifestValidator::validate(&manifest).is_ok());
    }

    #[test]
    fn test_validate_name_starts_with_hyphen() {
        assert!(ManifestValidator::validate_name("-invalid").is_err());
    }

    #[test]
    fn test_validate_name_starts_with_underscore() {
        assert!(ManifestValidator::validate_name("_invalid").is_err());
    }

    #[test]
    fn test_validate_version_format_one_part() {
        assert!(ManifestValidator::validate_version_format("1").is_ok());
    }

    #[test]
    fn test_validate_version_format_two_parts() {
        assert!(ManifestValidator::validate_version_format("1.2").is_ok());
    }

    #[test]
    fn test_validate_lua_version_long() {
        let long = ">=".to_string() + &"5".repeat(101);
        assert!(ManifestValidator::validate_lua_version(&long).is_err());
    }

    #[test]
    fn test_validate_build_config_builtin() {
        use crate::package::manifest::BuildConfig;
        let build = BuildConfig {
            build_type: "builtin".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: Vec::new(),
            profile: None,
        };
        assert!(ManifestValidator::validate_build_config(&Some(build)).is_ok());
    }

    #[test]
    fn test_validate_build_config_none() {
        use crate::package::manifest::BuildConfig;
        let build = BuildConfig {
            build_type: "none".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: Vec::new(),
            profile: None,
        };
        assert!(ManifestValidator::validate_build_config(&Some(build)).is_ok());
    }

    #[test]
    fn test_validate_build_config_rust_without_manifest() {
        use crate::package::manifest::BuildConfig;
        let mut modules = std::collections::HashMap::new();
        modules.insert("mymodule".to_string(), "libmymodule.so".to_string());
        let build = BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules,
            features: Vec::new(),
            profile: None,
        };
        // Should be ok even without manifest path
        assert!(ManifestValidator::validate_build_config(&Some(build)).is_ok());
    }

    #[test]
    fn test_validate_build_config_rust_wrong_manifest() {
        use crate::package::manifest::BuildConfig;
        let mut modules = std::collections::HashMap::new();
        modules.insert("mymodule".to_string(), "libmymodule.so".to_string());
        let build = BuildConfig {
            build_type: "rust".to_string(),
            manifest: Some("wrong.toml".to_string()),
            modules,
            features: Vec::new(),
            profile: None,
        };
        assert!(ManifestValidator::validate_build_config(&Some(build)).is_err());
    }

    #[test]
    fn test_validate_scripts_reserved_prepublish() {
        use std::collections::HashMap;
        let mut scripts_reserved = HashMap::new();
        scripts_reserved.insert("prepublish".to_string(), "command".to_string());
        assert!(ManifestValidator::validate_scripts(&scripts_reserved).is_err());
    }

    #[test]
    fn test_validate_scripts_reserved_postinstall() {
        use std::collections::HashMap;
        let mut scripts_reserved = HashMap::new();
        scripts_reserved.insert("postinstall".to_string(), "command".to_string());
        assert!(ManifestValidator::validate_scripts(&scripts_reserved).is_err());
    }

    #[test]
    fn test_validate_version_format_invalid_v2() {
        assert!(ManifestValidator::validate_version_format("1.2.3.4.5").is_err());
        assert!(ManifestValidator::validate_version_format("a.b.c").is_err());
    }

    #[test]
    fn test_validate_dependencies_invalid_name_v2() {
        use std::collections::HashMap;
        let mut deps = HashMap::new();
        deps.insert("-invalid".to_string(), "1.0.0".to_string());
        assert!(ManifestValidator::validate_dependencies(&deps).is_err());
    }

    #[test]
    fn test_validate_lua_version_valid_constraints() {
        assert!(ManifestValidator::validate_lua_version(">=5.1").is_ok());
        assert!(ManifestValidator::validate_lua_version(">5.1").is_ok());
        assert!(ManifestValidator::validate_lua_version("~5.1").is_ok());
    }
}
