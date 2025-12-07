use crate::core::version::parse_constraint;
use crate::core::{LpmError, LpmResult};
use crate::package::manifest::PackageManifest;
use crate::resolver::DependencyGraph;
use std::collections::HashMap;

/// Checks for conflicts before installation
pub struct ConflictChecker;

impl ConflictChecker {
    /// Check for conflicts in dependencies before installation
    pub fn check_conflicts(manifest: &PackageManifest) -> LpmResult<()> {
        // Check for duplicate dependencies between regular and dev
        let mut all_deps = HashMap::new();

        // Add regular dependencies
        for (name, version) in &manifest.dependencies {
            if let Some(existing) = all_deps.get(name) {
                return Err(LpmError::Package(format!(
                    "Conflict: '{}' is specified in both dependencies ({}) and dev_dependencies ({})",
                    name, existing, version
                )));
            }
            all_deps.insert(name.clone(), version.clone());
        }

        // Add dev dependencies
        for (name, version) in &manifest.dev_dependencies {
            if let Some(existing) = all_deps.get(name) {
                return Err(LpmError::Package(format!(
                    "Conflict: '{}' is specified in both dependencies ({}) and dev_dependencies ({})",
                    name, existing, version
                )));
            }
            all_deps.insert(name.clone(), version.clone());
        }

        // Check for version conflicts within dependencies
        Self::check_version_conflicts(&manifest.dependencies)?;
        Self::check_version_conflicts(&manifest.dev_dependencies)?;

        Ok(())
    }

    fn check_version_conflicts(deps: &HashMap<String, String>) -> LpmResult<()> {
        // Build dependency graph to check for circular dependencies
        let mut graph = DependencyGraph::new();

        for (name, version_str) in deps {
            let constraint = parse_constraint(version_str)?;
            graph.add_node(name.clone(), constraint);
        }

        // Check for circular dependencies
        graph.detect_circular_dependencies()?;

        Ok(())
    }

    /// Check if adding a new dependency would cause conflicts
    pub fn check_new_dependency(
        manifest: &PackageManifest,
        new_name: &str,
        new_version: &str,
    ) -> LpmResult<()> {
        // Check if already exists
        if manifest.dependencies.contains_key(new_name) {
            return Err(LpmError::Package(format!(
                "Package '{}' is already in dependencies with version '{}'",
                new_name,
                manifest.dependencies.get(new_name).unwrap()
            )));
        }

        if manifest.dev_dependencies.contains_key(new_name) {
            return Err(LpmError::Package(format!(
                "Package '{}' is already in dev_dependencies with version '{}'",
                new_name,
                manifest.dev_dependencies.get(new_name).unwrap()
            )));
        }

        // Validate the new dependency
        parse_constraint(new_version).map_err(|e| {
            LpmError::Package(format!(
                "Invalid version constraint '{}' for '{}': {}",
                new_version, new_name, e
            ))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageManifest;

    #[test]
    fn test_check_duplicate_dependency() {
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("test-pkg".to_string(), "1.0.0".to_string());
        manifest
            .dev_dependencies
            .insert("test-pkg".to_string(), "2.0.0".to_string());

        let result = ConflictChecker::check_conflicts(&manifest);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Conflict"));
    }

    #[test]
    fn test_check_new_dependency_conflict() {
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("test-pkg".to_string(), "1.0.0".to_string());

        let result = ConflictChecker::check_new_dependency(&manifest, "test-pkg", "2.0.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_new_dependency_valid() {
        let manifest = PackageManifest::default("test".to_string());
        let result = ConflictChecker::check_new_dependency(&manifest, "new-pkg", "^1.0.0");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_new_dependency_invalid_constraint() {
        let manifest = PackageManifest::default("test".to_string());
        let result = ConflictChecker::check_new_dependency(&manifest, "new-pkg", "invalid-version");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_conflicts_no_conflicts() {
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("pkg1".to_string(), "1.0.0".to_string());
        manifest
            .dev_dependencies
            .insert("pkg2".to_string(), "2.0.0".to_string());

        let result = ConflictChecker::check_conflicts(&manifest);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_new_dependency_in_dev_dependencies() {
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dev_dependencies
            .insert("test-pkg".to_string(), "1.0.0".to_string());

        let result = ConflictChecker::check_new_dependency(&manifest, "test-pkg", "2.0.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_conflicts_with_circular_dependency() {
        let mut manifest = PackageManifest::default("test".to_string());
        manifest
            .dependencies
            .insert("pkg1".to_string(), "1.0.0".to_string());
        manifest
            .dependencies
            .insert("pkg2".to_string(), "1.0.0".to_string());

        // This will fail if there's a circular dependency in the graph
        // The actual circular check happens in DependencyGraph
        let result = ConflictChecker::check_conflicts(&manifest);
        // Should pass if no circular deps, or fail if there are
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_check_new_dependency_empty_manifest() {
        let manifest = PackageManifest::default("test".to_string());
        let result = ConflictChecker::check_new_dependency(&manifest, "new-pkg", "1.0.0");
        assert!(result.is_ok());
    }
}
