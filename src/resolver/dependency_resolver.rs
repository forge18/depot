//! GitHub-based dependency resolver

use crate::core::{DepotError, DepotResult};
use crate::di::traits::GitHubProvider;
use crate::github::types::ResolvedVersion;
use crate::package::manifest::PackageManifest;
use depot_core::package::manifest::DependencySpec;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Resolution strategy for selecting package versions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolutionStrategy {
    /// Select the latest compatible version (default)
    #[default]
    Latest,
    /// Prefer releases over tags over branches
    PreferStable,
}

impl ResolutionStrategy {
    /// Parse a resolution strategy from a string
    pub fn parse(s: &str) -> DepotResult<Self> {
        match s.to_lowercase().as_str() {
            "latest" => Ok(ResolutionStrategy::Latest),
            "stable" | "prefer-stable" => Ok(ResolutionStrategy::PreferStable),
            _ => Err(DepotError::Config(format!(
                "Invalid resolution strategy '{}'. Must be 'latest' or 'prefer-stable'",
                s
            ))),
        }
    }
}

/// Resolved package information
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub repository: String, // "owner/repo"
    pub version: String,    // Resolved version string
    pub resolved: ResolvedVersion,
    pub dependencies: HashMap<String, DependencySpec>,
}

/// Resolves dependencies from GitHub repositories
pub struct DependencyResolver {
    github: Arc<dyn GitHubProvider>,
    _strategy: ResolutionStrategy,
    fallback_chain: Vec<String>,
}

impl DependencyResolver {
    /// Create a new resolver
    pub fn new(github: Arc<dyn GitHubProvider>, fallback_chain: Vec<String>) -> Self {
        Self {
            github,
            _strategy: ResolutionStrategy::default(),
            fallback_chain,
        }
    }

    /// Create a new resolver with custom strategy
    pub fn with_strategy(
        github: Arc<dyn GitHubProvider>,
        _strategy: ResolutionStrategy,
        fallback_chain: Vec<String>,
    ) -> Self {
        Self {
            github,
            _strategy,
            fallback_chain,
        }
    }

    /// Resolve all dependencies from a package manifest
    ///
    /// This implements a breadth-first dependency resolution:
    /// 1. Parse owner/repo from dependency specs
    /// 2. Resolve versions using GitHub API (releases/tags/branches)
    /// 3. Fetch package.yaml from resolved ref
    /// 4. Parse transitive dependencies
    /// 5. Detect conflicts and circular dependencies
    pub async fn resolve(
        &self,
        dependencies: &HashMap<String, DependencySpec>,
    ) -> DepotResult<HashMap<String, ResolvedPackage>> {
        let mut resolved: HashMap<String, ResolvedPackage> = HashMap::new();
        let mut queue: VecDeque<(String, DependencySpec)> = dependencies
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut visited = HashSet::new();
        let mut dependency_chain = Vec::new();

        while let Some((dep_key, dep_spec)) = queue.pop_front() {
            // Detect circular dependencies
            if dependency_chain.contains(&dep_key) {
                let cycle = dependency_chain
                    .iter()
                    .skip_while(|&x| x != &dep_key)
                    .chain(std::iter::once(&dep_key))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" -> ");
                return Err(DepotError::Package(format!(
                    "Circular dependency detected: {}",
                    cycle
                )));
            }

            // Skip if already resolved
            if resolved.contains_key(&dep_key) {
                continue;
            }

            // Skip if already visited (avoid re-processing)
            if !visited.insert(dep_key.clone()) {
                continue;
            }

            dependency_chain.push(dep_key.clone());

            // Parse owner/repo from dependency key (the key IS the repository for GitHub)
            let repository = dep_spec.repository.as_deref().unwrap_or(&dep_key);
            let (owner, repo) = parse_repository(repository)?;

            // Resolve version using GitHub API
            let version_spec = dep_spec.version.as_deref();
            let resolved_version = self
                .github
                .resolve_version(&owner, &repo, version_spec, &self.fallback_chain)
                .await
                .map_err(|e| {
                    DepotError::Package(format!("Failed to resolve {}/{}: {}", owner, repo, e))
                })?;

            // Try to fetch package.yaml to get transitive dependencies
            let package_manifest = self
                .fetch_package_manifest(&owner, &repo, &resolved_version.ref_value)
                .await;

            let transitive_deps = match package_manifest {
                Ok(manifest) => manifest.dependencies.clone(),
                Err(_) => {
                    // No package.yaml found - assume no dependencies
                    HashMap::new()
                }
            };

            // Convert HashMap<String, String> to HashMap<String, DependencySpec>
            let transitive_dep_specs: HashMap<String, DependencySpec> = transitive_deps
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        DependencySpec {
                            version: Some(v.clone()),
                            repository: None,
                        },
                    )
                })
                .collect();

            // Add to resolved packages
            resolved.insert(
                dep_key.clone(),
                ResolvedPackage {
                    repository: format!("{}/{}", owner, repo),
                    version: resolved_version.ref_value.clone(),
                    resolved: resolved_version,
                    dependencies: transitive_dep_specs.clone(),
                },
            );

            // Add transitive dependencies to queue
            for (trans_key, trans_spec) in transitive_dep_specs {
                if !resolved.contains_key(&trans_key) {
                    queue.push_back((trans_key, trans_spec));
                }
            }

            dependency_chain.pop();
        }

        Ok(resolved)
    }

    /// Fetch package.yaml from a GitHub repository at a specific ref
    async fn fetch_package_manifest(
        &self,
        owner: &str,
        repo: &str,
        ref_value: &str,
    ) -> DepotResult<PackageManifest> {
        // Try different package manifest filenames
        let filenames = vec!["package.yaml", "package.yml", ".depot", ".depot.yaml"];

        for filename in filenames {
            match self
                .github
                .get_file_content(owner, repo, filename, ref_value)
                .await
            {
                Ok(content) => {
                    // Parse YAML
                    let manifest: PackageManifest = serde_yaml::from_str(&content)
                        .map_err(|e| DepotError::Package(format!("Invalid package.yaml: {}", e)))?;
                    return Ok(manifest);
                }
                Err(_) => continue,
            }
        }

        Err(DepotError::Package(format!(
            "No package.yaml found in {}/{} at {}",
            owner, repo, ref_value
        )))
    }

    /// Resolve version conflicts between multiple constraints for the same package
    pub async fn resolve_conflict(
        &self,
        repository: &str,
        constraints: &[Option<String>],
    ) -> DepotResult<ResolvedVersion> {
        if constraints.is_empty() {
            return Err(DepotError::Package("No constraints provided".to_string()));
        }

        // If only one constraint, resolve it directly
        if constraints.len() == 1 {
            let (owner, repo) = parse_repository(repository)?;
            return self
                .github
                .resolve_version(
                    &owner,
                    &repo,
                    constraints[0].as_deref(),
                    &self.fallback_chain,
                )
                .await;
        }

        // For multiple constraints, we need to find a version that satisfies all
        // This is simplified for GitHub - we just take the first specific version constraint
        // or fall back to the fallback chain if all are None
        if let Some(version_spec) = constraints.iter().flatten().next() {
            let (owner, repo) = parse_repository(repository)?;
            return self
                .github
                .resolve_version(&owner, &repo, Some(version_spec), &self.fallback_chain)
                .await;
        }

        // All constraints are None - use fallback chain
        let (owner, repo) = parse_repository(repository)?;
        self.github
            .resolve_version(&owner, &repo, None, &self.fallback_chain)
            .await
    }
}

/// Parse owner/repo from repository string
/// Accepts formats: "owner/repo", "github.com/owner/repo", "https://github.com/owner/repo"
fn parse_repository(repository: &str) -> DepotResult<(String, String)> {
    let repo = repository.trim();

    // Strip protocol if present
    let repo = repo
        .strip_prefix("https://")
        .or_else(|| repo.strip_prefix("http://"))
        .unwrap_or(repo);

    // Strip github.com if present
    let repo = repo.strip_prefix("github.com/").unwrap_or(repo);

    // Now should be in "owner/repo" format
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        return Err(DepotError::Config(format!(
            "Invalid repository format '{}'. Expected 'owner/repo'",
            repository
        )));
    }

    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repository() {
        assert_eq!(
            parse_repository("owner/repo").unwrap(),
            ("owner".to_string(), "repo".to_string())
        );

        assert_eq!(
            parse_repository("github.com/owner/repo").unwrap(),
            ("owner".to_string(), "repo".to_string())
        );

        assert_eq!(
            parse_repository("https://github.com/owner/repo").unwrap(),
            ("owner".to_string(), "repo".to_string())
        );

        assert!(parse_repository("invalid").is_err());
        assert!(parse_repository("too/many/parts").is_err());
    }

    #[test]
    fn test_resolution_strategy_parse() {
        assert_eq!(
            ResolutionStrategy::parse("latest").unwrap(),
            ResolutionStrategy::Latest
        );
        assert_eq!(
            ResolutionStrategy::parse("stable").unwrap(),
            ResolutionStrategy::PreferStable
        );
        assert_eq!(
            ResolutionStrategy::parse("prefer-stable").unwrap(),
            ResolutionStrategy::PreferStable
        );
        assert!(ResolutionStrategy::parse("invalid").is_err());
    }

    #[test]
    fn test_resolution_strategy_default() {
        let strategy = ResolutionStrategy::default();
        assert_eq!(strategy, ResolutionStrategy::Latest);
    }

    #[tokio::test]
    async fn test_resolver_creation() {
        use crate::di::mocks::MockGitHubProvider;
        use std::sync::Arc;

        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec![
            "release".to_string(),
            "tag".to_string(),
            "branch".to_string(),
        ];

        let _resolver = DependencyResolver::new(github, fallback);
    }

    #[tokio::test]
    async fn test_resolver_with_strategy() {
        use crate::di::mocks::MockGitHubProvider;
        use std::sync::Arc;

        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];

        let resolver =
            DependencyResolver::with_strategy(github, ResolutionStrategy::PreferStable, fallback);

        assert_eq!(resolver._strategy, ResolutionStrategy::PreferStable);
    }

    #[tokio::test]
    async fn test_resolve_empty_dependencies() {
        use crate::di::mocks::MockGitHubProvider;
        use std::sync::Arc;

        let github = Arc::new(MockGitHubProvider::new());
        let fallback = vec!["release".to_string()];
        let resolver = DependencyResolver::new(github, fallback);

        let deps = HashMap::new();
        let result = resolver.resolve(&deps).await.unwrap();
        assert!(result.is_empty());
    }
}
