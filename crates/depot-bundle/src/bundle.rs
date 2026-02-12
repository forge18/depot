use crate::bundler::{BundleOptions, Bundler};
use depot_core::core::path::find_project_root;
use depot_core::{DepotError, DepotResult};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BundleRunOptions {
    pub entry: Option<String>,
    pub output: Option<String>,
    pub minify: bool,
    pub source_map: bool,
    pub no_comments: bool,
    pub tree_shake: bool,
    pub dynamic_requires: bool,
    pub incremental: bool,
}

pub fn run_with_options(opts: BundleRunOptions) -> DepotResult<()> {
    let current_dir = env::current_dir()?;

    // Find project root
    let project_root = find_project_root(&current_dir)?;

    // Resolve entry path (relative to project root or absolute)
    let entry_path = if let Some(entry_str) = opts.entry {
        let pb = PathBuf::from(&entry_str);
        if pb.is_absolute() {
            pb
        } else {
            project_root.join(pb)
        }
    } else {
        project_root.join("src/main.lua")
    };

    // Check if entry exists
    if !entry_path.exists() {
        return Err(DepotError::Package(format!(
            "Entry file not found: {}",
            entry_path.display()
        )));
    }

    // Resolve output path
    let output_path = if let Some(output_str) = opts.output {
        let pb = PathBuf::from(&output_str);
        if pb.is_absolute() {
            pb
        } else {
            project_root.join(pb)
        }
    } else {
        project_root.join("dist/bundle.lua")
    };

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let options = BundleOptions {
        minify: opts.minify,
        source_map: opts.source_map,
        comments: !opts.no_comments,
        standalone: true,
        module_paths: vec![project_root.join("src"), project_root.join("lib")],
        tree_shake: opts.tree_shake,
        dynamic_requires: opts.dynamic_requires,
        incremental: opts.incremental,
    };

    let bundler = Bundler::new(project_root, entry_path, output_path, options);
    bundler.bundle()?;

    Ok(())
}

/// Watch mode: automatically rebundle on file changes
pub fn run_watch(
    entry: Option<String>,
    output: Option<String>,
    minify: bool,
    source_map: bool,
    tree_shake: bool,
) -> DepotResult<()> {
    use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
    use std::sync::mpsc;
    use std::time::Duration;

    let current_dir = env::current_dir()?;
    let project_root = find_project_root(&current_dir)?;

    // Resolve entry and output paths (same as run())
    let entry_path = if let Some(entry_str) = entry {
        let pb = PathBuf::from(&entry_str);
        if pb.is_absolute() {
            pb
        } else {
            project_root.join(pb)
        }
    } else {
        project_root.join("src/main.lua")
    };

    let output_path = if let Some(output_str) = output {
        let pb = PathBuf::from(&output_str);
        if pb.is_absolute() {
            pb
        } else {
            project_root.join(pb)
        }
    } else {
        project_root.join("dist/bundle.lua")
    };

    if !entry_path.exists() {
        return Err(DepotError::Package(format!(
            "Entry file not found: {}",
            entry_path.display()
        )));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let options = BundleOptions {
        minify,
        source_map,
        comments: true,
        standalone: true,
        module_paths: vec![project_root.join("src"), project_root.join("lib")],
        tree_shake,
        dynamic_requires: false,
        incremental: true, // Enable incremental for watch mode
    };

    println!("👀 Watching for changes...");
    println!("   Entry: {}", entry_path.display());
    println!("   Output: {}", output_path.display());
    println!("   Press Ctrl+C to stop\n");

    // Initial bundle
    let bundler = Bundler::new(
        project_root.clone(),
        entry_path.clone(),
        output_path.clone(),
        options.clone(),
    );
    bundler.bundle()?;

    // Set up file watcher
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(500), None, tx)
        .map_err(|e| DepotError::Package(format!("Failed to create file watcher: {}", e)))?;

    // Watch source directories
    debouncer
        .watcher()
        .watch(&project_root.join("src"), RecursiveMode::Recursive)
        .map_err(|e| DepotError::Package(format!("Failed to watch src/: {}", e)))?;

    if project_root.join("lib").exists() {
        debouncer
            .watcher()
            .watch(&project_root.join("lib"), RecursiveMode::Recursive)
            .map_err(|e| DepotError::Package(format!("Failed to watch lib/: {}", e)))?;
    }

    // Watch for changes
    for result in rx {
        match result {
            Ok(events) => {
                if !events.is_empty() {
                    println!("\n📝 File change detected, rebundling...");
                    let bundler = Bundler::new(
                        project_root.clone(),
                        entry_path.clone(),
                        output_path.clone(),
                        options.clone(),
                    );
                    if let Err(e) = bundler.bundle() {
                        eprintln!("❌ Error bundling: {}", e);
                    } else {
                        println!("✓ Bundle updated\n");
                    }
                }
            }
            Err(e) => eprintln!("⚠️  Watch error: {:?}", e),
        }
    }

    Ok(())
}
