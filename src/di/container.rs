//! Service container for dependency injection

use super::traits::{CacheProvider, ConfigProvider, PackageClient, SearchProvider};
use crate::cache::Cache;
use crate::config::Config;
use crate::core::LpmResult;
use crate::luarocks::client::LuaRocksClient;
use crate::luarocks::search_api::SearchAPI;
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
/// use lpm::di::ServiceContainer;
///
/// # fn example() -> lpm::core::LpmResult<()> {
/// let container = ServiceContainer::new()?;
/// let config = container.config();
/// println!("Manifest URL: {}", config.luarocks_manifest_url());
/// # Ok(())
/// # }
/// ```
///
/// # Example (Testing)
///
/// ```ignore
/// use lpm::di::{ServiceContainer, mocks::*};
/// use std::sync::Arc;
///
/// # fn example() {
/// let config = Arc::new(MockConfigProvider::default());
/// let cache = Arc::new(MockCacheProvider::new());
/// let client = Arc::new(MockPackageClient::new());
/// let search = Arc::new(MockSearchProvider::new());
///
/// let container = ServiceContainer::with_providers(config, cache, client, search);
/// # }
/// ```
#[derive(Clone)]
pub struct ServiceContainer {
    pub config: Arc<dyn ConfigProvider>,
    pub cache: Arc<dyn CacheProvider>,
    pub package_client: Arc<dyn PackageClient>,
    pub search_provider: Arc<dyn SearchProvider>,
}

impl ServiceContainer {
    /// Create a new service container with production implementations
    ///
    /// This creates instances of all core services using the real implementations:
    /// - Loads config from disk
    /// - Creates cache in the configured directory
    /// - Initializes HTTP clients for LuaRocks
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Config file cannot be loaded or created
    /// - Cache directory cannot be created
    pub fn new() -> LpmResult<Self> {
        let config = Config::load()?;
        let cache = Cache::new(config.get_cache_dir()?)?;

        Ok(Self {
            config: Arc::new(config.clone()),
            cache: Arc::new(cache.clone()),
            package_client: Arc::new(LuaRocksClient::new(&config, cache.clone())),
            search_provider: Arc::new(SearchAPI::new()),
        })
    }

    /// Create a service container with custom provider implementations
    ///
    /// This is primarily useful for testing, where you can inject mock
    /// implementations of each service.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use lpm::di::{ServiceContainer, mocks::*};
    /// use std::sync::Arc;
    ///
    /// # fn example() {
    /// let config = Arc::new(MockConfigProvider::default());
    /// let cache = Arc::new(MockCacheProvider::new());
    /// let client = Arc::new(MockPackageClient::new());
    /// let search = Arc::new(MockSearchProvider::new());
    ///
    /// let container = ServiceContainer::with_providers(
    ///     config,
    ///     cache,
    ///     client,
    ///     search,
    /// );
    /// # }
    /// ```
    pub fn with_providers(
        config: Arc<dyn ConfigProvider>,
        cache: Arc<dyn CacheProvider>,
        package_client: Arc<dyn PackageClient>,
        search_provider: Arc<dyn SearchProvider>,
    ) -> Self {
        Self {
            config,
            cache,
            package_client,
            search_provider,
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

    /// Get the package client
    pub fn package_client(&self) -> &dyn PackageClient {
        self.package_client.as_ref()
    }

    /// Get the search provider
    pub fn search_provider(&self) -> &dyn SearchProvider {
        self.search_provider.as_ref()
    }
}
