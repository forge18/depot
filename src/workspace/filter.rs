use crate::core::DepotResult;
use crate::workspace::workspace_config::{Workspace, WorkspacePackage};
use std::collections::HashSet;

/// Filter for selecting packages within a workspace
#[derive(Debug, Clone)]
pub struct WorkspaceFilter {
    patterns: Vec<FilterPattern>,
}

#[derive(Debug, Clone)]
enum FilterPattern {
    /// Exact package name
    Exact(String),
    /// Glob pattern (e.g., "packages/*")
    Glob(String),
    /// Package and its dependents (e.g., "...my-package")
    WithDependents(String),
    /// Package and its dependencies (e.g., "my-package...")
    WithDependencies(String),
}

impl WorkspaceFilter {
    /// Create a new filter from patterns
    pub fn new(patterns: Vec<String>) -> Self {
        let parsed_patterns = patterns
            .into_iter()
            .map(|p| FilterPattern::parse(&p))
            .collect();

        Self {
            patterns: parsed_patterns,
        }
    }

    /// Create a filter that matches all packages
    pub fn all() -> Self {
        Self {
            patterns: vec![FilterPattern::Glob("*".to_string())],
        }
    }

    /// Check if a package name matches any pattern
    pub fn matches(&self, package_name: &str) -> bool {
        if self.patterns.is_empty() {
            return true; // No filter means match everything
        }

        self.patterns.iter().any(|p| p.matches_name(package_name))
    }

    /// Filter packages from a workspace based on patterns
    pub fn filter_packages<'a>(
        &self,
        workspace: &'a Workspace,
    ) -> DepotResult<Vec<&'a WorkspacePackage>> {
        if self.patterns.is_empty() {
            // No filter - return all packages
            return Ok(workspace.packages.values().collect());
        }

        let mut selected = HashSet::new();

        for pattern in &self.patterns {
            match pattern {
                FilterPattern::Exact(name) => {
                    if workspace.packages.contains_key(name) {
                        selected.insert(name.clone());
                    }
                }
                FilterPattern::Glob(pattern_str) => {
                    for name in workspace.packages.keys() {
                        if glob_match(pattern_str, name) {
                            selected.insert(name.clone());
                        }
                    }
                }
                FilterPattern::WithDependents(name) => {
                    // Include the package itself
                    if workspace.packages.contains_key(name) {
                        selected.insert(name.clone());
                    }

                    // Include all packages that depend on this one
                    for (pkg_name, pkg) in &workspace.packages {
                        if pkg.manifest.dependencies.contains_key(name)
                            || pkg.manifest.dev_dependencies.contains_key(name)
                        {
                            selected.insert(pkg_name.clone());
                        }
                    }
                }
                FilterPattern::WithDependencies(name) => {
                    // Include the package itself
                    if let Some(pkg) = workspace.packages.get(name) {
                        selected.insert(name.clone());

                        // Include all its dependencies (only workspace packages)
                        Self::collect_dependencies(&mut selected, pkg, workspace);
                    }
                }
            }
        }

        // Convert selected names to package references
        Ok(selected
            .iter()
            .filter_map(|name| workspace.packages.get(name))
            .collect())
    }

    /// Recursively collect all dependencies of a package (only workspace packages)
    fn collect_dependencies(
        selected: &mut HashSet<String>,
        package: &WorkspacePackage,
        workspace: &Workspace,
    ) {
        // Check both regular and dev dependencies
        for dep_name in package
            .manifest
            .dependencies
            .keys()
            .chain(package.manifest.dev_dependencies.keys())
        {
            // Only include if it's a workspace package
            if workspace.packages.contains_key(dep_name) && !selected.contains(dep_name) {
                selected.insert(dep_name.clone());

                // Recursively collect dependencies
                if let Some(dep_pkg) = workspace.packages.get(dep_name) {
                    Self::collect_dependencies(selected, dep_pkg, workspace);
                }
            }
        }
    }
}

impl FilterPattern {
    /// Parse a filter pattern from a string
    fn parse(s: &str) -> Self {
        if s.starts_with("...") {
            // ...package - package and its dependents
            Self::WithDependents(s.trim_start_matches("...").to_string())
        } else if s.ends_with("...") {
            // package... - package and its dependencies
            Self::WithDependencies(s.trim_end_matches("...").to_string())
        } else if s.contains('*') || s.contains('?') {
            // Glob pattern
            Self::Glob(s.to_string())
        } else {
            // Exact name
            Self::Exact(s.to_string())
        }
    }

    /// Check if this pattern matches a package name
    fn matches_name(&self, name: &str) -> bool {
        match self {
            Self::Exact(pattern) => pattern == name,
            Self::Glob(pattern) => glob_match(pattern, name),
            // WithDependents and WithDependencies are handled in filter_packages
            Self::WithDependents(_) | Self::WithDependencies(_) => false,
        }
    }
}

/// Simple glob matching supporting * and ? wildcards
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    glob_match_recursive(&pattern_chars, &text_chars, 0, 0)
}

fn glob_match_recursive(pattern: &[char], text: &[char], p_idx: usize, t_idx: usize) -> bool {
    // Base cases
    if p_idx == pattern.len() && t_idx == text.len() {
        return true;
    }
    if p_idx == pattern.len() {
        return false;
    }

    match pattern[p_idx] {
        '*' => {
            // Try matching zero or more characters
            if glob_match_recursive(pattern, text, p_idx + 1, t_idx) {
                return true;
            }
            if t_idx < text.len() && glob_match_recursive(pattern, text, p_idx, t_idx + 1) {
                return true;
            }
            false
        }
        '?' => {
            // Match exactly one character
            if t_idx < text.len() {
                glob_match_recursive(pattern, text, p_idx + 1, t_idx + 1)
            } else {
                false
            }
        }
        c => {
            // Match exact character
            if t_idx < text.len() && text[t_idx] == c {
                glob_match_recursive(pattern, text, p_idx + 1, t_idx + 1)
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::manifest::PackageManifest;
    use crate::workspace::workspace_config::WorkspaceConfig;
    use std::path::PathBuf;

    fn create_test_workspace() -> Workspace {
        let mut packages = std::collections::HashMap::new();

        // Package A (no dependencies)
        packages.insert(
            "package-a".to_string(),
            WorkspacePackage {
                name: "package-a".to_string(),
                path: PathBuf::from("packages/a"),
                manifest: PackageManifest::default("package-a".to_string()),
            },
        );

        // Package B (depends on A)
        let mut manifest_b = PackageManifest::default("package-b".to_string());
        manifest_b
            .dependencies
            .insert("package-a".to_string(), "^1.0.0".to_string());
        packages.insert(
            "package-b".to_string(),
            WorkspacePackage {
                name: "package-b".to_string(),
                path: PathBuf::from("packages/b"),
                manifest: manifest_b,
            },
        );

        // Package C (depends on B)
        let mut manifest_c = PackageManifest::default("package-c".to_string());
        manifest_c
            .dependencies
            .insert("package-b".to_string(), "^1.0.0".to_string());
        packages.insert(
            "package-c".to_string(),
            WorkspacePackage {
                name: "package-c".to_string(),
                path: PathBuf::from("packages/c"),
                manifest: manifest_c,
            },
        );

        Workspace {
            root: PathBuf::from("/workspace"),
            config: WorkspaceConfig {
                name: "test-workspace".to_string(),
                packages: vec!["packages/*".to_string()],
                ..Default::default()
            },
            packages,
        }
    }

    #[test]
    fn test_filter_exact_match() {
        let workspace = create_test_workspace();
        let filter = WorkspaceFilter::new(vec!["package-a".to_string()]);

        let filtered = filter.filter_packages(&workspace).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "package-a");
    }

    #[test]
    fn test_filter_glob_pattern() {
        let workspace = create_test_workspace();
        let filter = WorkspaceFilter::new(vec!["package-*".to_string()]);

        let filtered = filter.filter_packages(&workspace).unwrap();
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_filter_with_dependents() {
        let workspace = create_test_workspace();
        let filter = WorkspaceFilter::new(vec!["...package-a".to_string()]);

        let filtered = filter.filter_packages(&workspace).unwrap();
        // Should include package-a and package-b (depends on a)
        assert!(filtered.len() >= 2);

        let names: Vec<String> = filtered.iter().map(|p| p.name.clone()).collect();
        assert!(names.contains(&"package-a".to_string()));
        assert!(names.contains(&"package-b".to_string()));
    }

    #[test]
    fn test_filter_with_dependencies() {
        let workspace = create_test_workspace();
        let filter = WorkspaceFilter::new(vec!["package-c...".to_string()]);

        let filtered = filter.filter_packages(&workspace).unwrap();
        // Should include package-c, package-b, and package-a (transitive)
        assert!(filtered.len() >= 2);

        let names: Vec<String> = filtered.iter().map(|p| p.name.clone()).collect();
        assert!(names.contains(&"package-c".to_string()));
        assert!(names.contains(&"package-b".to_string()));
    }

    #[test]
    fn test_filter_no_patterns() {
        let workspace = create_test_workspace();
        let filter = WorkspaceFilter::new(vec![]);

        let filtered = filter.filter_packages(&workspace).unwrap();
        assert_eq!(filtered.len(), 3); // All packages
    }

    #[test]
    fn test_filter_all() {
        let workspace = create_test_workspace();
        let filter = WorkspaceFilter::all();

        let filtered = filter.filter_packages(&workspace).unwrap();
        assert_eq!(filtered.len(), 3); // All packages
    }

    #[test]
    fn test_filter_matches() {
        let filter = WorkspaceFilter::new(vec!["package-a".to_string()]);
        assert!(filter.matches("package-a"));
        assert!(!filter.matches("package-b"));
    }

    #[test]
    fn test_filter_pattern_parse() {
        assert!(matches!(
            FilterPattern::parse("package-a"),
            FilterPattern::Exact(_)
        ));
        assert!(matches!(
            FilterPattern::parse("package-*"),
            FilterPattern::Glob(_)
        ));
        assert!(matches!(
            FilterPattern::parse("...package-a"),
            FilterPattern::WithDependents(_)
        ));
        assert!(matches!(
            FilterPattern::parse("package-a..."),
            FilterPattern::WithDependencies(_)
        ));
    }
}
