//! Dependency injection infrastructure for Depot
//!
//! This module provides trait-based dependency injection to improve testability,
//! reduce coupling, and enable better architectural boundaries.
//!
//! # Example (Production)
//! ```no_run
//! use depot::di::ServiceContainer;
//!
//! # fn example() -> depot::core::DepotResult<()> {
//! let container = ServiceContainer::new()?;
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
//! let github = Arc::new(MockGitHubProvider::new());
//!
//! let container = ServiceContainer::with_providers(config, cache, github);
//! # }
//! ```

pub mod container;
pub mod mocks;
pub mod traits;

// Re-export key types
pub use container::ServiceContainer;
pub use traits::{CacheProvider, ConfigProvider, GitHubProvider};
