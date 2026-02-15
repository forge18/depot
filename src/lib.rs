//! Depot (Local Package Manager) for Lua
//!
//! This crate provides the main Depot library, re-exporting core functionality
//! from `depot-core` and organizing additional modules for package management
//! and related features.

pub use depot_core::package::manifest::PackageManifest;
pub use depot_core::path_setup::{LuaRunner, PathSetup, RunOptions};
pub use depot_core::{format_error_with_help, CredentialStore, DepotError, DepotResult, ErrorHelp};

/// Core module re-exported for backward compatibility.
pub mod core {
    pub use depot_core::core::*;
    pub use depot_core::*;

    /// Path module re-exported from depot-core.
    pub mod path {
        pub use depot_core::core::path::*;
    }

    /// Path setup for Depot binary (not Lua paths).
    pub mod path_setup;
}

/// Configuration management.
pub mod config;

/// Package caching.
pub mod cache;

/// Package management (install, update, remove).
pub mod package;

/// GitHub integration for package sources.
pub mod github;

/// Path setup and Lua runner (re-exported from depot-core).
pub mod path_setup {
    pub use depot_core::path_setup::*;
}

/// Rust extension building.
pub mod build;

/// Workspace support.
pub mod workspace;

/// Security and auditing.
pub mod security;

/// Lua version support.
pub mod lua_version;

/// Lua version manager.
pub mod lua_manager;

/// Dependency injection infrastructure.
pub mod di;

/// Dependency resolution.
pub mod resolver;

/// Lua source code analysis for version compatibility.
pub mod lua_analysis;
