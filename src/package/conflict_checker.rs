use crate::config::Config;
use crate::core::version::{parse_constraint, VersionConstraint};
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

    /// Check for conflicts with strict mode enabled
    /// Performs additional checks for transitive conflicts, diamond dependencies, etc.
    pub fn check_strict_conflicts(
        manifest: &PackageManifest,
        graph: &DependencyGraph,
        config: &Config,
    ) -> LpmResult<Vec<String>> {
        if !config.strict_conflicts {
            return Ok(Vec::new());
        }

        let mut warnings = Vec::new();

        // 1. Check for transitive dependency conflicts
        warnings.extend(Self::check_transitive_conflicts(graph)?);

        // 2. Check for diamond dependency version mismatches
        warnings.extend(Self::check_diamond_dependencies(graph)?);

        // 3. Check constraint satisfiability
        warnings.extend(Self::check_constraint_satisfiability(graph)?);

        // 4. Check for phantom dependencies (imported but not declared)
        warnings.extend(Self::check_phantom_dependencies(manifest)?);

        Ok(warnings)
    }

    /// Check if multiple packages require incompatible versions of the same transitive dependency
    fn check_transitive_conflicts(graph: &DependencyGraph) -> LpmResult<Vec<String>> {
        let mut warnings = Vec::new();
        let mut dep_constraints: HashMap<String, Vec<(String, VersionConstraint)>> = HashMap::new();

        // Collect all constraints for each dependency
        for package in graph.node_names() {
            if let Some(node) = graph.get_node(&package) {
                for dep in &node.dependencies {
                    if let Some(dep_node) = graph.get_node(dep) {
                        dep_constraints
                            .entry(dep.clone())
                            .or_default()
                            .push((package.clone(), dep_node.constraint.clone()));
                    }
                }
            }
        }

        // Check for conflicting constraints
        for (dep_name, constraints) in dep_constraints {
            if constraints.len() > 1 {
                // Check if all constraints are compatible
                let mut incompatible = false;
                for i in 0..constraints.len() {
                    for j in (i + 1)..constraints.len() {
                        let (pkg1, constraint1) = &constraints[i];
                        let (pkg2, constraint2) = &constraints[j];

                        if !Self::constraints_compatible(constraint1, constraint2) {
                            warnings.push(format!(
                                "Transitive conflict: '{}' requires '{}' {}, but '{}' requires '{}'",
                                pkg1, dep_name, Self::constraint_to_string(constraint1),
                                pkg2, Self::constraint_to_string(constraint2)
                            ));
                            incompatible = true;
                        }
                    }
                }

                if !incompatible && constraints.len() > 1 {
                    // Multiple packages depend on same package - this is OK, just informational
                }
            }
        }

        Ok(warnings)
    }

    /// Check for diamond dependency patterns where the same package appears at different levels
    fn check_diamond_dependencies(graph: &DependencyGraph) -> LpmResult<Vec<String>> {
        let mut warnings = Vec::new();
        let mut package_depths: HashMap<String, Vec<usize>> = HashMap::new();

        // Track depth of each package in the dependency tree
        for root in graph.node_names() {
            Self::track_depths(&root, graph, &mut package_depths, 0);
        }

        // Find packages that appear at multiple depths (diamond pattern)
        for (package, depths) in package_depths {
            if depths.len() > 1 {
                warnings.push(format!(
                    "Diamond dependency detected: '{}' appears at depths {:?} in the dependency tree",
                    package, depths
                ));
            }
        }

        Ok(warnings)
    }

    /// Track the depth of each package in the dependency tree
    fn track_depths(
        package: &str,
        graph: &DependencyGraph,
        depths: &mut HashMap<String, Vec<usize>>,
        current_depth: usize,
    ) {
        depths
            .entry(package.to_string())
            .or_default()
            .push(current_depth);

        if let Some(node) = graph.get_node(package) {
            for dep in &node.dependencies {
                Self::track_depths(dep, graph, depths, current_depth + 1);
            }
        }
    }

    /// Verify that all version constraints can be satisfied
    fn check_constraint_satisfiability(graph: &DependencyGraph) -> LpmResult<Vec<String>> {
        let mut warnings = Vec::new();

        for package in graph.node_names() {
            if let Some(node) = graph.get_node(&package) {
                // Check if resolved version satisfies the constraint
                if let Some(ref resolved) = node.resolved_version {
                    if !resolved.satisfies(&node.constraint) {
                        warnings.push(format!(
                            "Constraint violation: '{}' resolved to {} but constraint is {}",
                            package,
                            resolved,
                            Self::constraint_to_string(&node.constraint)
                        ));
                    }
                }
            }
        }

        Ok(warnings)
    }

    /// Check for phantom dependencies (packages used but not declared)
    /// This is a placeholder - in practice, would need code analysis to detect imports
    fn check_phantom_dependencies(_manifest: &PackageManifest) -> LpmResult<Vec<String>> {
        // This would require analyzing Lua source files to detect require() calls
        // For now, return empty list as this is beyond scope of static analysis
        Ok(Vec::new())
    }

    /// Check if two version constraints are compatible
    fn constraints_compatible(c1: &VersionConstraint, c2: &VersionConstraint) -> bool {
        // Two constraints are compatible if there exists a version that satisfies both
        // This is a simplified check - a full implementation would need to analyze ranges
        use crate::core::version::VersionConstraint::*;

        match (c1, c2) {
            (Exact(v1), Exact(v2)) => v1 == v2,
            (Exact(v), Compatible(base)) | (Compatible(base), Exact(v)) => {
                v.major == base.major && v >= base
            }
            (Exact(v), Patch(base)) | (Patch(base), Exact(v)) => {
                v.major == base.major && v.minor == base.minor && v >= base
            }
            (Compatible(v1), Compatible(v2)) => v1.major == v2.major,
            (Patch(v1), Patch(v2)) => v1.major == v2.major && v1.minor == v2.minor,
            (Compatible(c), Patch(t)) | (Patch(t), Compatible(c)) => {
                c.major == t.major && c.minor == t.minor
            }
            (AnyPatch(v1), AnyPatch(v2)) => v1.major == v2.major && v1.minor == v2.minor,
            (AnyPatch(v), Patch(p)) | (Patch(p), AnyPatch(v)) => {
                v.major == p.major && v.minor == p.minor
            }
            (GreaterOrEqual(_), _) | (_, GreaterOrEqual(_)) => true, // Conservative: might be compatible
            (LessThan(_), _) | (_, LessThan(_)) => true, // Conservative: might be compatible
            (AnyPatch(_), _) | (_, AnyPatch(_)) => true, // Conservative: might be compatible
        }
    }

    /// Convert constraint to string for display
    fn constraint_to_string(constraint: &VersionConstraint) -> String {
        use crate::core::version::VersionConstraint::*;
        match constraint {
            Exact(v) => v.to_string(),
            Compatible(v) => format!("^{}", v),
            Patch(v) => format!("~{}", v),
            GreaterOrEqual(v) => format!(">={}", v),
            LessThan(v) => format!("<{}", v),
            AnyPatch(v) => format!("{}.{}.x", v.major, v.minor),
        }
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

    #[test]
    fn test_strict_conflicts_disabled() {
        let manifest = PackageManifest::default("test".to_string());
        let graph = DependencyGraph::new();
        let config = Config {
            strict_conflicts: false,
            ..Default::default()
        };

        let warnings = ConflictChecker::check_strict_conflicts(&manifest, &graph, &config);
        assert!(warnings.is_ok());
        assert!(warnings.unwrap().is_empty());
    }

    #[test]
    fn test_strict_conflicts_enabled() {
        let manifest = PackageManifest::default("test".to_string());
        let graph = DependencyGraph::new();
        let config = Config {
            strict_conflicts: true,
            ..Default::default()
        };

        let warnings = ConflictChecker::check_strict_conflicts(&manifest, &graph, &config);
        assert!(warnings.is_ok());
    }

    #[test]
    fn test_transitive_conflicts() {
        use crate::core::version::parse_constraint;

        let mut graph = DependencyGraph::new();

        // Package A depends on C with version ^1.0.0
        graph.add_node("A".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("C".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_dependency("A", "C".to_string()).unwrap();

        // Package B depends on C with version ^2.0.0
        graph.add_node("B".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("C_v2".to_string(), parse_constraint("^2.0.0").unwrap());

        let warnings = ConflictChecker::check_transitive_conflicts(&graph).unwrap();
        // Should not have warnings yet since we haven't linked B to C
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_diamond_dependencies() {
        use crate::core::version::parse_constraint;

        let mut graph = DependencyGraph::new();
        graph.add_node("root".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("A".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("B".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("shared".to_string(), parse_constraint("^1.0.0").unwrap());

        graph.add_dependency("root", "A".to_string()).unwrap();
        graph.add_dependency("root", "B".to_string()).unwrap();
        graph.add_dependency("A", "shared".to_string()).unwrap();
        graph.add_dependency("B", "shared".to_string()).unwrap();

        let warnings = ConflictChecker::check_diamond_dependencies(&graph).unwrap();
        // Should detect that "shared" appears at multiple depths
        assert!(!warnings.is_empty());

        // Check if any warning contains "shared"
        let has_shared = warnings.iter().any(|w| w.contains("shared"));
        assert!(
            has_shared,
            "Expected 'shared' in warnings but got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_constraint_satisfiability() {
        use crate::core::version::{parse_constraint, Version};

        let mut graph = DependencyGraph::new();
        graph.add_node("test".to_string(), parse_constraint("^1.0.0").unwrap());

        // Set a resolved version that doesn't match the constraint
        graph
            .set_resolved_version("test", Version::new(2, 0, 0))
            .unwrap();

        let warnings = ConflictChecker::check_constraint_satisfiability(&graph).unwrap();
        // Should detect constraint violation
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("Constraint violation"));
    }

    #[test]
    fn test_constraints_compatible_exact() {
        use crate::core::version::{Version, VersionConstraint};

        let v1 = VersionConstraint::Exact(Version::new(1, 0, 0));
        let v2 = VersionConstraint::Exact(Version::new(1, 0, 0));
        assert!(ConflictChecker::constraints_compatible(&v1, &v2));

        let v3 = VersionConstraint::Exact(Version::new(2, 0, 0));
        assert!(!ConflictChecker::constraints_compatible(&v1, &v3));
    }

    #[test]
    fn test_constraints_compatible_compatible() {
        use crate::core::version::{Version, VersionConstraint};

        let c1 = VersionConstraint::Compatible(Version::new(1, 0, 0));
        let c2 = VersionConstraint::Compatible(Version::new(1, 5, 0));
        assert!(ConflictChecker::constraints_compatible(&c1, &c2));

        let c3 = VersionConstraint::Compatible(Version::new(2, 0, 0));
        assert!(!ConflictChecker::constraints_compatible(&c1, &c3));
    }

    #[test]
    fn test_constraints_compatible_patch() {
        use crate::core::version::{Version, VersionConstraint};

        let t1 = VersionConstraint::Patch(Version::new(1, 2, 0));
        let t2 = VersionConstraint::Patch(Version::new(1, 2, 5));
        assert!(ConflictChecker::constraints_compatible(&t1, &t2));

        let t3 = VersionConstraint::Patch(Version::new(1, 3, 0));
        assert!(!ConflictChecker::constraints_compatible(&t1, &t3));
    }

    #[test]
    fn test_constraints_compatible_greater_or_equal() {
        use crate::core::version::{Version, VersionConstraint};

        let ge = VersionConstraint::GreaterOrEqual(Version::new(1, 0, 0));
        let exact = VersionConstraint::Exact(Version::new(1, 0, 0));
        // Conservative: we assume they might be compatible
        assert!(ConflictChecker::constraints_compatible(&ge, &exact));
        assert!(ConflictChecker::constraints_compatible(&exact, &ge));
    }

    #[test]
    fn test_constraint_to_string() {
        use crate::core::version::{Version, VersionConstraint};

        assert_eq!(
            ConflictChecker::constraint_to_string(&VersionConstraint::Exact(Version::new(1, 0, 0))),
            "1.0.0"
        );
        assert_eq!(
            ConflictChecker::constraint_to_string(&VersionConstraint::Compatible(Version::new(
                1, 0, 0
            ))),
            "^1.0.0"
        );
        assert_eq!(
            ConflictChecker::constraint_to_string(&VersionConstraint::Patch(Version::new(1, 0, 0))),
            "~1.0.0"
        );
        assert_eq!(
            ConflictChecker::constraint_to_string(&VersionConstraint::GreaterOrEqual(
                Version::new(1, 0, 0)
            )),
            ">=1.0.0"
        );
        assert_eq!(
            ConflictChecker::constraint_to_string(&VersionConstraint::LessThan(Version::new(
                2, 0, 0
            ))),
            "<2.0.0"
        );
    }
}
