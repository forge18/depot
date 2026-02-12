use crate::core::path::lua_modules_dir;
use crate::core::DepotResult;
use std::fs;
use std::path::{Path, PathBuf};

/// Generates the lpm.loader Lua module that sets up package.path and package.cpath
pub struct PathSetup;

impl PathSetup {
    /// Generate the lpm.loader module content
    ///
    /// This generates version-aware loader code that works with both
    /// Lua 5.1 (package.loaders) and Lua 5.2+ (package.searchers)
    pub fn generate_loader(project_root: &Path) -> String {
        let lua_modules = lua_modules_dir(project_root);
        let lua_modules_str = lua_modules.to_string_lossy();

        // Generate platform-specific cpath extension
        let cpath_extension = if cfg!(target_os = "windows") {
            format!("{}/?.dll;{}/?/init.dll", lua_modules_str, lua_modules_str)
        } else if cfg!(target_os = "macos") {
            format!(
                "{}/?.dylib;{}/?/init.dylib",
                lua_modules_str, lua_modules_str
            )
        } else {
            // Linux and other Unix-like systems
            format!("{}/?.so;{}/?/init.so", lua_modules_str, lua_modules_str)
        };

        format!(
            r#"-- Depot Loader Module
-- Automatically sets up package.path and package.cpath for local dependencies
-- Compatible with Lua 5.1, 5.3, and 5.4

local lua_modules = [[{}]]

-- Add lua_modules to package.path
-- Supports both ?/init.lua and ?.lua patterns
local lpm_path = lua_modules .. [[/?/init.lua;]] ..
                 lua_modules .. [[/?.lua;]] ..
                 lua_modules .. [[/?/?.lua;]]

-- Add lua_modules to package.cpath for native modules
local lpm_cpath = [[{}]]

-- Prepend Depot paths to existing paths
package.path = lpm_path .. package.path
package.cpath = lpm_cpath .. package.cpath

-- Lua 5.1 compatibility: package.loaders vs package.searchers
-- Lua 5.1 uses package.loaders, Lua 5.2+ uses package.searchers
-- This loader works with both by modifying package.path/cpath directly
-- which is compatible with all versions

-- Return a table with utility functions
return {{
    lua_modules = lua_modules,
    path = lpm_path,
    cpath = lpm_cpath,
}}
"#,
            lua_modules_str, cpath_extension
        )
    }

    /// Install the lpm.loader module to lua_modules/lpm/loader.lua
    /// This allows it to be required as "lpm.loader"
    pub fn install_loader(project_root: &Path) -> DepotResult<()> {
        let loader_content = Self::generate_loader(project_root);
        let lpm_dir = lua_modules_dir(project_root).join("lpm");
        let loader_path = lpm_dir.join("loader.lua");

        // Ensure lpm directory exists
        fs::create_dir_all(&lpm_dir)?;

        fs::write(&loader_path, loader_content)?;
        Ok(())
    }

    /// Get the path to the installed loader module
    pub fn loader_path(project_root: &Path) -> PathBuf {
        lua_modules_dir(project_root).join("lpm").join("loader.lua")
    }

    /// Get the directory containing the lpm module
    pub fn lpm_module_dir(project_root: &Path) -> PathBuf {
        lua_modules_dir(project_root).join("lpm")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_loader() {
        let temp = TempDir::new().unwrap();
        let loader = PathSetup::generate_loader(temp.path());

        assert!(loader.contains("package.path"));
        assert!(loader.contains("package.cpath"));
        assert!(loader.contains("lua_modules"));
    }

    #[test]
    fn test_install_loader() {
        let temp = TempDir::new().unwrap();
        PathSetup::install_loader(temp.path()).unwrap();

        let loader_path = PathSetup::loader_path(temp.path());
        assert!(loader_path.exists());
    }
}
