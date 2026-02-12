use crate::bundler::minifier::Minifier;
use crate::bundler::resolver::DependencyResolver;
use crate::bundler::tree_shaker::TreeShaker;
use depot_core::{DepotError, DepotResult};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Bundle Lua files into a single file
pub struct Bundler {
    /// Root directory
    root: PathBuf,
    /// Entry point file
    entry: PathBuf,
    /// Output file
    output: PathBuf,
    /// Options
    options: BundleOptions,
}

#[derive(Debug, Clone)]
pub struct BundleOptions {
    /// Minify output
    pub minify: bool,
    /// Generate source map
    pub source_map: bool,
    /// Include comments
    pub comments: bool,
    /// Standalone (include runtime)
    pub standalone: bool,
    /// Custom module paths
    pub module_paths: Vec<PathBuf>,
    /// Enable tree-shaking (remove unused code)
    pub tree_shake: bool,
    /// Support dynamic requires (runtime analysis)
    pub dynamic_requires: bool,
    /// Incremental bundling (only rebuild changed modules)
    pub incremental: bool,
}

impl Default for BundleOptions {
    fn default() -> Self {
        Self {
            minify: false,
            source_map: false,
            comments: true,
            standalone: true,
            module_paths: vec![PathBuf::from("src"), PathBuf::from("lib")],
            tree_shake: false,
            dynamic_requires: false,
            incremental: false,
        }
    }
}

impl Bundler {
    pub fn new(root: PathBuf, entry: PathBuf, output: PathBuf, options: BundleOptions) -> Self {
        Self {
            root,
            entry,
            output,
            options,
        }
    }

    /// Bundle all dependencies into a single file
    pub fn bundle(&self) -> DepotResult<()> {
        println!("📦 Bundling {}...", self.entry.display());

        // Check if we can use incremental bundling
        let should_rebuild = if self.options.incremental {
            self.should_rebuild()?
        } else {
            true
        };

        if !should_rebuild && self.options.incremental {
            println!("   No changes detected, skipping rebuild (incremental mode)");
            return Ok(());
        }

        // 1. Resolve all dependencies
        let mut resolver = DependencyResolver::new(&self.root, &self.options.module_paths);

        let modules = resolver.resolve(&self.entry)?;
        println!("   Found {} modules", modules.len());

        // 2. Apply tree-shaking if enabled
        let modules_to_bundle = if self.options.tree_shake {
            println!("   Applying tree-shaking...");
            let tree_shaker = TreeShaker::new();
            let entry_module_name = self.get_entry_module_name()?;
            tree_shaker.shake(&modules, &entry_module_name)?
        } else {
            // Read all modules
            let mut all_modules = HashMap::new();
            for (module_name, module_path) in &modules {
                let content = fs::read_to_string(module_path)?;
                all_modules.insert(module_name.clone(), content);
            }
            all_modules
        };

        // 3. Read and process each module
        let mut bundled_modules: HashMap<String, String> = HashMap::new();

        for (module_name, content) in modules_to_bundle {
            let processed = if self.options.minify {
                let minifier = Minifier::new();
                minifier.minify(&content)?
            } else if !self.options.comments {
                self.strip_comments(&content)
            } else {
                content
            };

            bundled_modules.insert(module_name.clone(), processed);
        }

        // 4. Generate bundle
        let bundle = self.generate_bundle(bundled_modules)?;

        // 5. Write output
        fs::write(&self.output, bundle)?;

        let size = fs::metadata(&self.output)?.len();
        println!(
            "✓ Bundle created: {} ({} bytes)",
            self.output.display(),
            size
        );

        // 6. Generate source map if requested
        if self.options.source_map {
            self.generate_source_map(&modules)?;
        }

        // 7. Warn about dynamic requires if any were found
        if self.options.dynamic_requires {
            let dynamic = resolver.dynamic_requires();
            if !dynamic.is_empty() {
                eprintln!("⚠️  Warning: Found {} dynamic require(s):", dynamic.len());
                for req in dynamic {
                    eprintln!("   - {}", req);
                }
                eprintln!("   These may not be included in the bundle.");
            }
        }

        Ok(())
    }

    /// Check if we should rebuild based on file modification times
    fn should_rebuild(&self) -> DepotResult<bool> {
        // Get output file modification time
        let output_mtime = if self.output.exists() {
            fs::metadata(&self.output)?.modified().ok()
        } else {
            // Output doesn't exist, must rebuild
            return Ok(true);
        };

        // Check if entry point is newer than output
        if self.entry.exists() {
            if let Ok(entry_mtime) = fs::metadata(&self.entry)?.modified() {
                if let Some(output_mtime) = output_mtime {
                    if entry_mtime > output_mtime {
                        return Ok(true);
                    }
                }
            }
        }

        // Check all source files in module paths
        for module_path in &self.options.module_paths {
            if module_path.exists() {
                if let Ok(entries) = fs::read_dir(module_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().and_then(|s| s.to_str()) == Some("lua")
                        {
                            if let Ok(file_mtime) = fs::metadata(&path)?.modified() {
                                if let Some(output_mtime) = output_mtime {
                                    if file_mtime > output_mtime {
                                        return Ok(true);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    fn generate_bundle(&self, modules: HashMap<String, String>) -> DepotResult<String> {
        let mut bundle = String::new();

        if self.options.standalone {
            // Add module loader runtime
            bundle.push_str(&self.get_runtime());
            bundle.push('\n');
        }

        // Add all modules
        for (name, content) in modules {
            bundle.push_str(&format!("\n-- Module: {}\n", name));
            // Escape single quotes in module name for safety
            let escaped_name = name.replace("'", "\\'");
            bundle.push_str(&format!(
                "_BUNDLER_RUNTIME.modules['{}'] = function()\n",
                escaped_name
            ));
            bundle.push_str(&content);
            bundle.push_str("\nend\n");
        }

        // Add entry point execution
        bundle.push_str("\n-- Entry point\n");
        bundle.push_str(&format!("require('{}')\n", self.get_entry_module_name()?));

        Ok(bundle)
    }

    fn get_runtime(&self) -> String {
        r#"-- Depot Bundler Runtime
-- This runtime provides a scoped require override for bundled modules
local _BUNDLER_RUNTIME = {
    modules = {},
    loaded = {},
    original_require = require,
}

-- Store original require before we override it
local original_require = _BUNDLER_RUNTIME.original_require

-- Create a scoped require function
local function bundled_require(name)
    -- Check if it's a bundled module
    if _BUNDLER_RUNTIME.modules[name] then
        if not _BUNDLER_RUNTIME.loaded[name] then
            local result = _BUNDLER_RUNTIME.modules[name]()
            _BUNDLER_RUNTIME.loaded[name] = result or true
        end
        return _BUNDLER_RUNTIME.loaded[name]
    end
    
    -- Fall back to original require for C modules, stdlib, etc.
    return original_require(name)
end

-- Override require in the global scope
-- This is safe because we preserve the original require
require = bundled_require

-- Also make the runtime available for debugging
__BUNDLER__ = _BUNDLER_RUNTIME
"#
        .to_string()
    }

    fn get_entry_module_name(&self) -> DepotResult<String> {
        let rel_path = self.entry.strip_prefix(&self.root).map_err(|_| {
            DepotError::Path(format!(
                "Entry path {} is not within project root {}",
                self.entry.display(),
                self.root.display()
            ))
        })?;

        let module_name = rel_path
            .to_string_lossy()
            .replace(".lua", "")
            .replace("/", ".")
            .replace("\\", ".");

        Ok(module_name)
    }

    fn strip_comments(&self, content: &str) -> String {
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.starts_with("--") || trimmed.starts_with("--[[")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn generate_source_map(&self, modules: &HashMap<String, PathBuf>) -> DepotResult<()> {
        let map = serde_json::json!({
            "version": 3,
            "file": self.output.file_name().unwrap().to_string_lossy(),
            "sources": modules.values().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
            "names": modules.keys().collect::<Vec<_>>(),
        });

        let map_path = self.output.with_extension("lua.map");
        let map_json = serde_json::to_string_pretty(&map)
            .map_err(|e| DepotError::Package(format!("Failed to serialize source map: {}", e)))?;
        fs::write(&map_path, map_json)?;

        println!("   Source map: {}", map_path.display());

        Ok(())
    }
}
