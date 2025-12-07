use crate::core::version::{parse_constraint, Version, VersionConstraint};
use crate::core::{LpmError, LpmResult};
use crate::luarocks::version::to_luarocks_version;
use crate::package::manifest::PackageManifest;

/// Generates rockspec files from package.yaml
pub struct RockspecGenerator;

impl RockspecGenerator {
    /// Generate a rockspec file content from a PackageManifest
    pub fn generate(manifest: &PackageManifest) -> LpmResult<String> {
        // Convert version to LuaRocks format (e.g., "1.2.3" -> "1.2-1")
        let luarocks_version = to_luarocks_version(&Version::parse(&manifest.version)?);

        let mut rockspec = String::new();

        // Header
        rockspec.push_str(&format!("package = \"{}\"\n", manifest.name));
        rockspec.push_str(&format!("version = \"{}\"\n", luarocks_version));
        rockspec.push('\n');

        // Description
        if let Some(desc) = &manifest.description {
            rockspec.push_str(&format!("description = \"{}\"\n", escape_lua_string(desc)));
        }

        // Homepage
        if let Some(homepage) = &manifest.homepage {
            rockspec.push_str(&format!("homepage = \"{}\"\n", homepage));
        }

        // License
        if let Some(license) = &manifest.license {
            rockspec.push_str(&format!("license = \"{}\"\n", license));
        }

        // Lua version
        rockspec.push_str(&format!("lua_version = \"{}\"\n", manifest.lua_version));
        rockspec.push('\n');

        // Source (for now, we'll use a placeholder - this should be set by the publisher)
        rockspec.push_str("source = {\n");
        rockspec.push_str("  url = \"\", -- Will be set during publish\n");
        rockspec.push_str("}\n");
        rockspec.push('\n');

        // Dependencies
        if !manifest.dependencies.is_empty() {
            rockspec.push_str("dependencies = {\n");
            for (name, version) in &manifest.dependencies {
                // Convert SemVer to LuaRocks format
                let luarocks_dep = Self::format_dependency(name, version)?;
                rockspec.push_str(&format!("  \"{}\",\n", luarocks_dep));
            }
            rockspec.push_str("}\n");
            rockspec.push('\n');
        }

        // Build configuration
        rockspec.push_str("build = {\n");
        if let Some(build) = &manifest.build {
            match build.build_type.as_str() {
                "rust" => {
                    rockspec.push_str("  type = \"builtin\",\n");
                    rockspec.push_str("  modules = {\n");
                    for (module_name, module_path) in &build.modules {
                        // Extract just the module name from the path
                        let module_file = module_path.rsplit('/').next().unwrap_or(module_name);
                        rockspec.push_str(&format!(
                            "    [\"{}\"] = \"{}\",\n",
                            module_name, module_file
                        ));
                    }
                    rockspec.push_str("  },\n");
                }
                "builtin" => {
                    rockspec.push_str("  type = \"builtin\",\n");
                }
                "none" => {
                    rockspec.push_str("  type = \"none\",\n");
                }
                _ => {
                    return Err(LpmError::Package(format!(
                        "Unsupported build type for rockspec: {}",
                        build.build_type
                    )));
                }
            }
        } else {
            rockspec.push_str("  type = \"builtin\",\n");
        }
        rockspec.push_str("}\n");

        Ok(rockspec)
    }

    /// Format a dependency in LuaRocks format
    fn format_dependency(name: &str, version: &str) -> LpmResult<String> {
        if version == "*" || version.is_empty() {
            return Ok(name.to_string());
        }

        // Parse SemVer constraint and convert to LuaRocks format
        let constraint = parse_constraint(version)?;

        match constraint {
            VersionConstraint::Exact(v) => {
                let luarocks_ver = to_luarocks_version(&v);
                Ok(format!("{} == {}", name, luarocks_ver))
            }
            VersionConstraint::Compatible(v) => {
                // ^1.2.3 -> ~>1.2.3
                let luarocks_ver = to_luarocks_version(&v);
                Ok(format!("{} ~> {}", name, luarocks_ver))
            }
            VersionConstraint::Patch(v) => {
                // ~1.2.3 -> ~>1.2.3
                let luarocks_ver = to_luarocks_version(&v);
                Ok(format!("{} ~> {}", name, luarocks_ver))
            }
            VersionConstraint::GreaterOrEqual(v) => {
                let luarocks_ver = to_luarocks_version(&v);
                Ok(format!("{} >= {}", name, luarocks_ver))
            }
            VersionConstraint::LessThan(v) => {
                let luarocks_ver = to_luarocks_version(&v);
                Ok(format!("{} < {}", name, luarocks_ver))
            }
            VersionConstraint::AnyPatch(v) => {
                let luarocks_ver = to_luarocks_version(&v);
                Ok(format!("{} ~> {}", name, luarocks_ver))
            }
        }
    }
}

/// Escape special characters in Lua strings
fn escape_lua_string(s: &str) -> String {
    s.replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\t", "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageManifest;

    #[test]
    fn test_generate_rockspec() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.version = "1.2.3".to_string();
        manifest.description = Some("Test package".to_string());
        manifest
            .dependencies
            .insert("luasocket".to_string(), "3.0.0".to_string());

        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        assert!(rockspec.contains("package = \"test-package\""));
        // Version "1.2.3" converts to "1.2-3" (patch version becomes revision)
        assert!(rockspec.contains("version = \"1.2-3\""));
        assert!(rockspec.contains("luasocket"));
    }

    #[test]
    fn test_generate_rockspec_with_dependencies() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.version = "1.0.0".to_string();
        manifest
            .dependencies
            .insert("luasocket".to_string(), "^3.0.0".to_string());
        manifest
            .dependencies
            .insert("penlight".to_string(), ">=1.0.0".to_string());

        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        assert!(rockspec.contains("dependencies = {"));
        assert!(rockspec.contains("luasocket"));
        assert!(rockspec.contains("penlight"));
    }

    #[test]
    fn test_generate_rockspec_with_metadata() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.description = Some("A test package".to_string());
        manifest.homepage = Some("https://example.com".to_string());
        manifest.license = Some("MIT".to_string());

        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        assert!(rockspec.contains("description = \"A test package\""));
        assert!(rockspec.contains("homepage = \"https://example.com\""));
        assert!(rockspec.contains("license = \"MIT\""));
    }

    #[test]
    fn test_escape_lua_string() {
        // Test that escape_lua_string handles special characters
        let test_cases = vec![
            ("normal text", "normal text"),
            ("text with \"quotes\"", "text with \\\"quotes\\\""),
            ("text with\nnewline", "text with\\nnewline"),
        ];

        for (input, expected) in test_cases {
            let escaped = escape_lua_string(input);
            assert_eq!(escaped, expected);
        }
    }

    #[test]
    fn test_format_dependency_wildcard() {
        let result = RockspecGenerator::format_dependency("test-pkg", "*").unwrap();
        assert_eq!(result, "test-pkg");
    }

    #[test]
    fn test_format_dependency_empty() {
        let result = RockspecGenerator::format_dependency("test-pkg", "").unwrap();
        assert_eq!(result, "test-pkg");
    }

    #[test]
    fn test_format_dependency_exact() {
        let result = RockspecGenerator::format_dependency("test-pkg", "1.2.3").unwrap();
        assert!(result.contains("test-pkg"));
        assert!(result.contains("=="));
    }

    #[test]
    fn test_format_dependency_compatible() {
        let result = RockspecGenerator::format_dependency("test-pkg", "^1.2.3").unwrap();
        assert!(result.contains("test-pkg"));
        assert!(result.contains("~>"));
    }

    #[test]
    fn test_format_dependency_patch() {
        let result = RockspecGenerator::format_dependency("test-pkg", "~1.2.3").unwrap();
        assert!(result.contains("test-pkg"));
        assert!(result.contains("~>"));
    }

    #[test]
    fn test_format_dependency_greater_or_equal() {
        let result = RockspecGenerator::format_dependency("test-pkg", ">=1.2.3").unwrap();
        assert!(result.contains("test-pkg"));
        assert!(result.contains(">="));
    }

    #[test]
    fn test_format_dependency_less_than() {
        let result = RockspecGenerator::format_dependency("test-pkg", "<2.0.0").unwrap();
        assert!(result.contains("test-pkg"));
        assert!(result.contains("<"));
    }

    #[test]
    fn test_generate_rockspec_with_build_rust() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.build = Some(crate::package::manifest::BuildConfig {
            build_type: "rust".to_string(),
            manifest: None,
            modules: vec![
                ("test_module".to_string(), "src/lib.rs".to_string()),
                ("other_module".to_string(), "src/other.rs".to_string()),
            ]
            .into_iter()
            .collect(),
            features: Vec::new(),
            profile: None,
        });

        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        assert!(rockspec.contains("type = \"builtin\""));
        assert!(rockspec.contains("modules = {"));
        assert!(rockspec.contains("test_module"));
        assert!(rockspec.contains("lib.rs"));
    }

    #[test]
    fn test_generate_rockspec_with_build_builtin() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.build = Some(crate::package::manifest::BuildConfig {
            build_type: "builtin".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: Vec::new(),
            profile: None,
        });

        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        assert!(rockspec.contains("type = \"builtin\""));
    }

    #[test]
    fn test_generate_rockspec_with_build_none() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.build = Some(crate::package::manifest::BuildConfig {
            build_type: "none".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: Vec::new(),
            profile: None,
        });

        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        assert!(rockspec.contains("type = \"none\""));
    }

    #[test]
    fn test_generate_rockspec_without_dependencies() {
        let manifest = PackageManifest::default("test-package".to_string());
        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        // Should not contain dependencies section if empty
        assert!(!rockspec.contains("dependencies = {"));
    }

    #[test]
    fn test_generate_rockspec_without_optional_fields() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.description = None;
        manifest.homepage = None;
        manifest.license = None;

        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        assert!(rockspec.contains("package = \"test-package\""));
        // Should still generate valid rockspec without optional fields
    }

    #[test]
    fn test_generate_rockspec_with_optional_fields() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.description = Some("Test description".to_string());
        manifest.homepage = Some("https://example.com".to_string());
        manifest.license = Some("MIT".to_string());
        let rockspec = RockspecGenerator::generate(&manifest).unwrap();
        assert!(rockspec.contains("description ="));
        assert!(rockspec.contains("homepage ="));
        assert!(rockspec.contains("license ="));
    }

    #[test]
    fn test_generate_rockspec_with_unsupported_build_type() {
        let mut manifest = PackageManifest::default("test-package".to_string());
        manifest.build = Some(crate::package::manifest::BuildConfig {
            build_type: "unsupported".to_string(),
            manifest: None,
            modules: std::collections::HashMap::new(),
            features: Vec::new(),
            profile: None,
        });
        let result = RockspecGenerator::generate(&manifest);
        assert!(result.is_err());
    }

    #[test]
    fn test_format_dependency_any_patch() {
        let result = RockspecGenerator::format_dependency("test-pkg", "~1.2.3").unwrap();
        assert!(result.contains("test-pkg"));
        assert!(result.contains("~>"));
    }

    #[test]
    fn test_escape_lua_string_special_chars() {
        let input = "test\"string\nwith\ttabs";
        let escaped = escape_lua_string(input);
        assert!(escaped.contains("\\\""));
        assert!(escaped.contains("\\n"));
        assert!(escaped.contains("\\t"));
    }
}
