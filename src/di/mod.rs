//! Dependency injection infrastructure for Depot
//!
//! This module provides trait-based dependency injection to improve testability,
//! reduce coupling, and enable better architectural boundaries.
//!
//! # Example (Production)
//! ```no_run
//! use depot::di::ServiceContainer;
//! use depot::package::installer::PackageInstaller;
//! use std::path::Path;
//!
//! # async fn example() -> depot::core::DepotResult<()> {
//! let container = ServiceContainer::new()?;
//! let installer = PackageInstaller::with_container(Path::new("."), container)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Example (Testing)
//! ```
//! use depot::di::{ServiceContainer, mocks::*};
//! use std::sync::Arc;
//!
//! # fn example() {
//! let config = Arc::new(MockConfigProvider::default());
//! let cache = Arc::new(MockCacheProvider::new());
//! let client = Arc::new(MockPackageClient::new());
//! let search = Arc::new(MockSearchProvider::new());
//!
//! let container = ServiceContainer::with_providers(config, cache, client, search);
//! # }
//! ```

pub mod container;
pub mod mocks;
pub mod traits;

// Re-export key types
pub use container::ServiceContainer;
pub use traits::{CacheProvider, ConfigProvider, GitHubProvider};
