//! Service container for dependency injection

use super::traits::{CacheProvider, ConfigProvider, GitHubProvider};
use crate::cache::Cache;
use crate::config::Config;
use crate::core::DepotResult;
use crate::github::GitHubClient;
use std::sync::Arc;

/// Service container for dependency injection
///
/// This container holds all core services and provides access to them
/// through trait objects. It uses `Arc<dyn Trait>` for runtime polymorphism,
/// allowing easy swapping of implementations for testing.
///
/// # Performance
///
/// The use of trait objects adds minimal overhead (one vtable lookup per call),
/// which is negligible compared to I/O operations (network, disk).
///
/// # Example (Production)
///
/// ```no_run
/// use depot::di::ServiceContainer;
///
/// # fn example() -> depot::core::DepotResult<()> {
/// let container = ServiceContainer::new()?;
/// let config = container.config();
/// println!("GitHub API URL: {}", config.github_api_url());
/// # Ok(())
/// # }
/// ```
///
/// # Example (Testing)
///
/// ```
/// use depot::di::{ServiceContainer, mocks::*};
/// use std::sync::Arc;
///
/// # fn example() {
/// let config = Arc::new(MockConfigProvider::default());
/// let cache = Arc::new(MockCacheProvider::new());
/// let github = Arc::new(MockGitHubProvider::new());
///
/// let container = ServiceContainer::with_providers(config, cache, github);
/// # }
/// ```
#[derive(Clone)]
pub struct ServiceContainer {
    pub config: Arc<dyn ConfigProvider>,
    pub cache: Arc<dyn CacheProvider>,
    pub github: Arc<dyn GitHubProvider>,
}

impl ServiceContainer {
    /// Create a new service container with production implementations
    ///
    /// This creates instances of all core services using the real implementations:
    /// - Loads config from disk
    /// - Creates cache in the configured directory
    /// - Initializes GitHub API client
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Config file cannot be loaded or created
    /// - Cache directory cannot be created
    /// - GitHub client cannot be initialized
    pub fn new() -> DepotResult<Self> {
        let config = Config::load()?;
        let cache = Cache::new(config.get_cache_dir()?)?;
        let cache_arc = Arc::new(cache.clone());

        Ok(Self {
            config: Arc::new(config.clone()),
            cache: cache_arc.clone(),
            github: Arc::new(GitHubClient::new(&config, cache_arc)?),
        })
    }

    /// Create a service container with custom provider implementations
    ///
    /// This is primarily useful for testing, where you can inject mock
    /// implementations of each service.
    ///
    /// # Example
    ///
    /// ```
    /// use depot::di::{ServiceContainer, mocks::*};
    /// use std::sync::Arc;
    ///
    /// # fn example() {
    /// let config = Arc::new(MockConfigProvider::default());
    /// let cache = Arc::new(MockCacheProvider::new());
    /// let github = Arc::new(MockGitHubProvider::new());
    ///
    /// let container = ServiceContainer::with_providers(
    ///     config,
    ///     cache,
    ///     github,
    /// );
    /// # }
    /// ```
    pub fn with_providers(
        config: Arc<dyn ConfigProvider>,
        cache: Arc<dyn CacheProvider>,
        github: Arc<dyn GitHubProvider>,
    ) -> Self {
        Self {
            config,
            cache,
            github,
        }
    }

    /// Get the configuration provider
    pub fn config(&self) -> &dyn ConfigProvider {
        self.config.as_ref()
    }

    /// Get the cache provider
    pub fn cache(&self) -> &dyn CacheProvider {
        self.cache.as_ref()
    }

    /// Get the GitHub provider
    pub fn github(&self) -> &dyn GitHubProvider {
        self.github.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::di::mocks::*;

    #[test]
    fn test_container_with_providers() {
        let config = Arc::new(MockConfigProvider::default());
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());

        let container =
            ServiceContainer::with_providers(config.clone(), cache.clone(), github.clone());

        // Verify providers are accessible
        assert_eq!(
            container.config().github_api_url(),
            "https://api.github.com"
        );
    }

    #[test]
    fn test_container_accessor_methods() {
        let config = Arc::new(MockConfigProvider::default());
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());

        let container =
            ServiceContainer::with_providers(config.clone(), cache.clone(), github.clone());

        // Test all accessor methods return valid trait objects
        let _ = container.config();
        let _ = container.cache();
        let _ = container.github();
    }

    #[test]
    fn test_container_config_provider() {
        let config = MockConfigProvider::default();

        let container = ServiceContainer::with_providers(
            Arc::new(config),
            Arc::new(MockCacheProvider::new()),
            Arc::new(MockGitHubProvider::new()),
        );

        assert_eq!(
            container.config().github_api_url(),
            "https://api.github.com"
        );
    }

    #[test]
    fn test_container_clone() {
        let config = Arc::new(MockConfigProvider::default());
        let cache = Arc::new(MockCacheProvider::new());
        let github = Arc::new(MockGitHubProvider::new());

        let container =
            ServiceContainer::with_providers(config.clone(), cache.clone(), github.clone());

        let cloned = container.clone();

        // Both should have access to the same providers
        assert_eq!(
            container.config().github_api_url(),
            cloned.config().github_api_url()
        );
    }

    #[test]
    fn test_container_cache_provider() {
        let cache = MockCacheProvider::new();
        let container = ServiceContainer::with_providers(
            Arc::new(MockConfigProvider::default()),
            Arc::new(cache),
            Arc::new(MockGitHubProvider::new()),
        );

        let provider = container.cache();
        assert!(!provider.exists(std::path::Path::new("/nonexistent")));
    }

    #[test]
    fn test_container_github_provider() {
        let github = MockGitHubProvider::new();

        let container = ServiceContainer::with_providers(
            Arc::new(MockConfigProvider::default()),
            Arc::new(MockCacheProvider::new()),
            Arc::new(github),
        );

        let _ = container.github();
    }
}
