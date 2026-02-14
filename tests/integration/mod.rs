//! Integration tests module
//!
//! This module contains all integration tests for Depot CLI commands.

// pub mod audit; // Removed - LuaRocks specific
pub mod build;
pub mod clean;
pub mod common;
pub mod error_recovery;
pub mod init;
pub mod install;
pub mod install_comprehensive;
pub mod interactive;
pub mod list;
pub mod login;
pub mod lua;
// pub mod outdated; // Removed - LuaRocks specific
pub mod package;
pub mod plugin;
pub mod publish;
pub mod remove;
pub mod run;
// pub mod security; // Removed - LuaRocks specific
pub mod template;
pub mod update;
pub mod verify;
