use crate::core::{DepotError, DepotResult};
use crate::lua_version::constraint::parse_lua_version_constraint;
use crate::lua_version::detector::LuaVersion;
use crate::luarocks::rockspec::Rockspec;

/// Checks package compatibility with Lua versions
pub struct PackageCompatibility;

impl PackageCompatibility {
    /// Check if a package is compatible with the installed Lua version
    pub fn check_package(
        installed_version: &LuaVersion,
        package_lua_version: Option<&str>,
    ) -> DepotResult<bool> {
        let package_constraint = if let Some(lua_version) = package_lua_version {
            parse_lua_version_constraint(lua_version)?
        } else {
            // No constraint specified, assume compatible
            return Ok(true);
        };

        Ok(package_constraint.matches(installed_version))
    }

    /// Check if a rockspec is compatible with the installed Lua version
    pub fn check_rockspec(
        installed_version: &LuaVersion,
        rockspec: &Rockspec,
    ) -> DepotResult<bool> {
        Self::check_package(installed_version, rockspec.lua_version.as_deref())
    }

    /// Validate that the project's lua_version constraint matches the installed version
    pub fn validate_project_constraint(
        installed_version: &LuaVersion,
        project_constraint: &str,
    ) -> DepotResult<()> {
        let constraint = parse_lua_version_constraint(project_constraint)?;

        if !constraint.matches(installed_version) {
            return Err(DepotError::Version(format!(
                "Installed Lua version {} does not satisfy project requirement '{}'",
                installed_version, project_constraint
            )));
        }

        Ok(())
    }

    /// Filter packages by Lua version compatibility
    pub fn filter_compatible_packages(
        installed_version: &LuaVersion,
        packages: &[(String, Option<String>)], // (name, lua_version)
    ) -> Vec<String> {
        packages
            .iter()
            .filter_map(|(name, lua_version)| {
                match Self::check_package(installed_version, lua_version.as_deref()) {
                    Ok(true) => Some(name.clone()),
                    Ok(false) => None,
                    Err(_) => None, // Skip on parse error
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_package_exact() {
        let installed = LuaVersion::new(5, 4, 0);
        assert!(PackageCompatibility::check_package(&installed, Some("5.4")).unwrap());
        assert!(!PackageCompatibility::check_package(&installed, Some("5.3")).unwrap());
    }

    #[test]
    fn test_check_package_range() {
        let installed = LuaVersion::new(5, 4, 0);
        assert!(PackageCompatibility::check_package(&installed, Some(">=5.1")).unwrap());
        assert!(!PackageCompatibility::check_package(&installed, Some("<5.3")).unwrap());
    }

    #[test]
    fn test_check_package_multiple() {
        let installed = LuaVersion::new(5, 3, 0);
        assert!(
            PackageCompatibility::check_package(&installed, Some("5.1 || 5.3 || 5.4")).unwrap()
        );
    }

    #[test]
    fn test_check_package_no_constraint() {
        let installed = LuaVersion::new(5, 4, 0);
        assert!(PackageCompatibility::check_package(&installed, None).unwrap());
    }

    #[test]
    fn test_check_rockspec() {
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        let installed = LuaVersion::new(5, 4, 0);
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "none".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: std::collections::HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: Some(">=5.1".to_string()),
            binary_urls: std::collections::HashMap::new(),
        };
        assert!(PackageCompatibility::check_rockspec(&installed, &rockspec).unwrap());
    }

    #[test]
    fn test_validate_project_constraint() {
        let installed = LuaVersion::new(5, 4, 0);
        assert!(PackageCompatibility::validate_project_constraint(&installed, ">=5.1").is_ok());
        assert!(PackageCompatibility::validate_project_constraint(&installed, "5.4").is_ok());
        assert!(PackageCompatibility::validate_project_constraint(&installed, "<5.3").is_err());
    }

    #[test]
    fn test_filter_compatible_packages() {
        let installed = LuaVersion::new(5, 4, 0);
        let packages = vec![
            ("pkg1".to_string(), Some(">=5.1".to_string())),
            ("pkg2".to_string(), Some("<5.3".to_string())),
            ("pkg3".to_string(), Some("5.4".to_string())),
            ("pkg4".to_string(), None),
        ];
        let compatible = PackageCompatibility::filter_compatible_packages(&installed, &packages);
        assert!(compatible.contains(&"pkg1".to_string()));
        assert!(!compatible.contains(&"pkg2".to_string()));
        assert!(compatible.contains(&"pkg3".to_string()));
        assert!(compatible.contains(&"pkg4".to_string()));
    }

    #[test]
    fn test_check_package_invalid_constraint() {
        let installed = LuaVersion::new(5, 4, 0);
        let result = PackageCompatibility::check_package(&installed, Some("invalid"));
        assert!(result.is_err());
    }

    #[test]
    fn test_check_rockspec_without_lua_version() {
        use crate::luarocks::rockspec::{Rockspec, RockspecBuild, RockspecSource};
        let installed = LuaVersion::new(5, 4, 0);
        let rockspec = Rockspec {
            package: "test".to_string(),
            version: "1.0.0".to_string(),
            source: RockspecSource {
                url: "none".to_string(),
                tag: None,
                branch: None,
            },
            dependencies: vec![],
            build: RockspecBuild {
                build_type: "builtin".to_string(),
                modules: std::collections::HashMap::new(),
                install: Default::default(),
            },
            description: None,
            homepage: None,
            license: None,
            lua_version: None,
            binary_urls: std::collections::HashMap::new(),
        };
        assert!(PackageCompatibility::check_rockspec(&installed, &rockspec).unwrap());
    }

    #[test]
    fn test_validate_project_constraint_invalid() {
        let installed = LuaVersion::new(5, 4, 0);
        let result = PackageCompatibility::validate_project_constraint(&installed, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_compatible_packages_with_errors() {
        let installed = LuaVersion::new(5, 4, 0);
        let packages = vec![
            ("pkg1".to_string(), Some(">=5.1".to_string())),
            ("pkg2".to_string(), Some("invalid-constraint".to_string())), // Should be skipped
            ("pkg3".to_string(), None),
        ];
        let compatible = PackageCompatibility::filter_compatible_packages(&installed, &packages);
        assert!(compatible.contains(&"pkg1".to_string()));
        assert!(!compatible.contains(&"pkg2".to_string()));
        assert!(compatible.contains(&"pkg3".to_string()));
    }

    #[test]
    fn test_filter_compatible_packages_empty() {
        let installed = LuaVersion::new(5, 4, 0);
        let packages: Vec<(String, Option<String>)> = vec![];
        let compatible = PackageCompatibility::filter_compatible_packages(&installed, &packages);
        assert!(compatible.is_empty());
    }
}
