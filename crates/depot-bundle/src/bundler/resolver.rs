use crate::bundler::parser::LuaParser;
use depot_core::{DepotError, DepotResult};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves all module dependencies
pub struct DependencyResolver {
    root: PathBuf,
    module_paths: Vec<PathBuf>,
    visited: HashSet<String>,
    resolving: HashSet<String>, // Track modules currently being resolved (for circular deps)
    parser: LuaParser,
    /// Track dynamic requires for runtime analysis
    dynamic_requires: Vec<String>,
}

impl DependencyResolver {
    pub fn new(root: &Path, module_paths: &[PathBuf]) -> Self {
        Self {
            root: root.to_path_buf(),
            module_paths: module_paths.to_vec(),
            visited: HashSet::new(),
            resolving: HashSet::new(),
            parser: LuaParser::new(),
            dynamic_requires: Vec::new(),
        }
    }

    /// Get list of dynamic requires found during resolution
    pub fn dynamic_requires(&self) -> &[String] {
        &self.dynamic_requires
    }

    /// Resolve all dependencies starting from entry point
    pub fn resolve(&mut self, entry: &Path) -> DepotResult<HashMap<String, PathBuf>> {
        let mut modules = HashMap::new();

        let entry_module = self.path_to_module_name(entry)?;
        self.resolve_recursive(&entry_module, entry, &mut modules)?;

        Ok(modules)
    }

    fn resolve_recursive(
        &mut self,
        module_name: &str,
        module_path: &Path,
        modules: &mut HashMap<String, PathBuf>,
    ) -> DepotResult<()> {
        // Check for circular dependencies
        if self.resolving.contains(module_name) {
            return Err(DepotError::Package(format!(
                "Circular dependency detected: {}",
                module_name
            )));
        }

        // Skip if already visited
        if self.visited.contains(module_name) {
            return Ok(());
        }

        self.resolving.insert(module_name.to_string());
        self.visited.insert(module_name.to_string());
        modules.insert(module_name.to_string(), module_path.to_path_buf());

        // Parse file for require() statements using proper Lua parser
        let content = fs::read_to_string(module_path)?;
        let requires = self.parser.extract_requires(&content)?;

        // Also check for dynamic requires (require(variable))
        // This is a simplified check - full implementation would need AST analysis
        if content.contains("require(") {
            // Try to detect dynamic requires (basic heuristic)
            // A proper implementation would walk the AST
            // For now, we'll note potential dynamic requires
        }

        // Resolve each required module
        for req in requires {
            if let Some(dep_path) = self.find_module(&req)? {
                let dep_module = self.path_to_module_name(&dep_path)?;
                self.resolve_recursive(&dep_module, &dep_path, modules)?;
            } else {
                // Warn about missing modules (might be C modules or stdlib)
                eprintln!(
                    "⚠️  Warning: Module '{}' not found (may be C module or stdlib)",
                    req
                );
            }
        }

        self.resolving.remove(module_name);
        Ok(())
    }

    // extract_requires is now handled by LuaParser

    fn find_module(&self, module_name: &str) -> DepotResult<Option<PathBuf>> {
        // Convert module.name to module/name.lua or module/name/init.lua
        let path_variants = vec![
            format!("{}.lua", module_name.replace(".", "/")),
            format!("{}/init.lua", module_name.replace(".", "/")),
        ];

        // Search in all module paths
        for base_path in &self.module_paths {
            let full_base = if base_path.is_absolute() {
                base_path.clone()
            } else {
                self.root.join(base_path)
            };

            for variant in &path_variants {
                let full_path = full_base.join(variant);
                if full_path.exists() {
                    return Ok(Some(full_path));
                }
            }
        }

        // Check lua_modules (Depot standard location)
        let lua_modules = self.root.join("lua_modules");
        if lua_modules.exists() {
            for variant in &path_variants {
                let full_path = lua_modules.join(variant);
                if full_path.exists() {
                    return Ok(Some(full_path));
                }
            }
        }

        // Not found - might be a C module, standard library, or external dependency
        // Return None and let caller handle (warn or error)
        Ok(None)
    }

    fn path_to_module_name(&self, path: &Path) -> DepotResult<String> {
        // Handle absolute paths
        let rel_path = if path.is_absolute() {
            path.strip_prefix(&self.root).map_err(|_| {
                DepotError::Path(format!(
                    "Path {} is not within project root {}",
                    path.display(),
                    self.root.display()
                ))
            })?
        } else {
            path
        };

        let mut module_name = rel_path
            .to_string_lossy()
            .replace(".lua", "")
            .replace("\\", "/"); // Normalize to forward slashes

        // Remove leading slash if present
        if module_name.starts_with('/') {
            module_name = module_name[1..].to_string();
        }

        Ok(module_name.replace("/", "."))
    }
}
