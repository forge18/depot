# Plugin Development Guide

Depot supports plugins as separate executables that extend the core functionality. This guide explains how to create and distribute Depot plugins.

## Overview

Plugins are standalone Rust binaries named `depot-<name>` that can be installed globally and are automatically discovered by the main `depot` CLI. When you run `depot <plugin-name>`, Depot will find and execute the corresponding `depot-<plugin-name>` binary.

## Plugin Discovery

Depot discovers plugins in two locations:

1. **Global installation directory**: `~/.depot/bin/lpm-<name>` (when installed via `depot install -g`)
2. **System PATH**: Any `depot-<name>` executable found in your PATH

The discovery mechanism is implemented in `src/cli/plugin.rs`:

```rust
pub fn find_plugin(plugin_name: &str) -> Option<PathBuf> {
    // Check ~/.depot/bin/lpm-{name}
    if let Ok(lpm_home) = lpm_home() {
        let plugin_path = lpm_home.join("bin").join(format!("lpm-{}", plugin_name));
        if plugin_path.exists() {
            return Some(plugin_path);
        }
    }
    
    // Check PATH for lpm-{name}
    which::which(format!("lpm-{}", plugin_name)).ok()
}
```

## Creating a Plugin

### 1. Project Structure

Create a new Rust project for your plugin:

```bash
cargo new --bin lpm-myplugin
cd lpm-myplugin
```

### 2. Cargo.toml

Your plugin's `Cargo.toml` should:

- Name the binary `depot-<name>`
- Depend on `depot-core` for shared utilities
- Include any plugin-specific dependencies

Example:

```toml
[package]
name = "lpm-myplugin"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "lpm-myplugin"
path = "src/main.rs"

[dependencies]
# Core Depot utilities
depot-core = { path = "../depot-core" }  # Or from crates.io when published

# CLI
clap = { version = "4.4", features = ["derive"] }

# Your plugin's dependencies
# ...
```

### 3. Main Entry Point

Your plugin's `main.rs` should:

- Use `clap` for CLI argument parsing
- Return `Result<(), lpm_core::LpmError>`
- Handle errors gracefully

Example:

```rust
use clap::{Parser, Subcommand};
use lpm_core::LpmError;

#[derive(Parser)]
#[command(name = "lpm-myplugin")]
#[command(about = "Description of your plugin")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Do something
    DoSomething {
        /// Some option
        #[arg(short, long)]
        option: Option<String>,
    },
}

fn main() -> Result<(), LpmError> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::DoSomething { option } => {
            // Your plugin logic here
            println!("Doing something with {:?}", option);
            Ok(())
        }
    }
}
```

## Using depot-core

The `depot-core` crate provides utilities that plugins can use:

### Error Handling

```rust
use lpm_core::{LpmError, LpmResult};

fn my_function() -> LpmResult<()> {
    // Use LpmError for errors
    Err(LpmError::Package("Something went wrong".to_string()))
}
```

### Path Utilities

```rust
use lpm_core::core::path::{find_project_root, lua_modules_dir};

// Find the project root (directory containing package.yaml)
let project_root = find_project_root(&std::env::current_dir()?)?;

// Get the lua_modules directory
let lua_modules = lua_modules_dir(&project_root);
```

### Package Manifest

```rust
use lpm_core::package::manifest::PackageManifest;

// Load package.yaml
let manifest = PackageManifest::load(&project_root)?;

// Access manifest fields
println!("Package: {}", manifest.name);
println!("Version: {}", manifest.version);
println!("Dependencies: {:?}", manifest.dependencies);
```

### Lua Runner

```rust
use lpm_core::path_setup::runner::{LuaRunner, RunOptions};

// Run a Lua script with proper path setup
let options = RunOptions {
    cwd: Some(project_root.to_string_lossy().to_string()),
    lua_args: vec!["script.lua".to_string()],
    env: vec![],
};

let exit_code = LuaRunner::run_script(&script_path, options)?;
```

### Path Setup

```rust
use lpm_core::path_setup::loader::PathSetup;

// Ensure lpm.loader is installed in the project
PathSetup::install_loader(&project_root)?;
```

## Example: Minimal Plugin

Here's a complete minimal plugin example:

```rust
use clap::Parser;
use lpm_core::{LpmError, LpmResult};
use lpm_core::core::path::find_project_root;

#[derive(Parser)]
#[command(name = "lpm-hello")]
#[command(about = "A simple hello world plugin")]
struct Cli {
    /// Name to greet
    #[arg(short, long, default_value = "world")]
    name: String,
}

fn main() -> Result<(), LpmError> {
    let cli = Cli::parse();
    
    // Find project root
    let current_dir = std::env::current_dir()?;
    let project_root = find_project_root(&current_dir)?;
    
    println!("Hello, {}!", cli.name);
    println!("Project root: {}", project_root.display());
    
    Ok(())
}
```

## Plugin Configuration in package.yaml

Plugins can read configuration from `package.yaml` by parsing the YAML directly:

```rust
use serde_yaml::Value;
use std::fs;

fn load_plugin_config(project_root: &Path) -> LpmResult<Option<Value>> {
    let package_yaml_path = project_root.join("package.yaml");
    if !package_yaml_path.exists() {
        return Ok(None);
    }
    
    let content = fs::read_to_string(&package_yaml_path)?;
    let yaml: Value = serde_yaml::from_str(&content)
        .map_err(|e| LpmError::Package(format!("Failed to parse package.yaml: {}", e)))?;
    
    // Access plugin-specific section
    Ok(yaml.get("myplugin").cloned())
}
```

Example `package.yaml`:

```yaml
name: my-project
version: 1.0.0

# Plugin-specific configuration
myplugin:
  setting1: value1
  setting2: value2
```

## Testing Your Plugin

### Local Development

During development, you can test your plugin by:

1. Building it:
   ```bash
   cargo build --release
   ```

2. Running it directly:
   ```bash
   ./target/release/lpm-myplugin
   ```

3. Or symlinking it to test discovery:
   ```bash
   ln -s $(pwd)/target/release/lpm-myplugin ~/.depot/bin/lpm-myplugin
   depot myplugin  # Should now work
   ```

### Integration Testing

Test that your plugin integrates correctly with Depot:

```bash
# Build and install globally
cargo build --release
cp target/release/lpm-myplugin ~/.depot/bin/

# Test discovery
depot myplugin --help

# Test execution
depot myplugin
```

## Distribution

### Publishing to crates.io

When `depot-core` is published to crates.io, plugins can depend on it:

```toml
[dependencies]
depot-core = "0.1.0"  # Version matching Depot release
```

### Installation

Users can install plugins globally:

```bash
# If published as a crate
depot install -g lpm-myplugin

# Or manually
cargo install lpm-myplugin
```

## Best Practices

1. **Error Handling**: Always return `LpmResult<()>` and use `LpmError` for errors
2. **Project Root**: Use `find_project_root()` to locate the Depot project
3. **Path Setup**: Use `LuaRunner` or `PathSetup` when running Lua code
4. **CLI Design**: Follow Depot's CLI conventions (use `clap`, clear help text)
5. **Documentation**: Document your plugin's usage and configuration
6. **Testing**: Test your plugin in real Depot projects

## Real-World Example

See `depot-watch` for a complete plugin implementation:

- File watching with `notify` and `notify-debouncer-mini`
- Configuration from `package.yaml`
- Integration with `depot-core` utilities
- Proper error handling and user feedback
- Multiple commands support
- Custom file type handlers
- WebSocket server for browser reload
- Enhanced terminal UI with colored output

## Limitations

- Plugins are separate processes, so they can't directly access Depot's internal state
- Plugin discovery happens at runtime, not compile time
- Plugins must be installed separately from the main Depot binary

## Questions?

- Check existing plugins for examples (`depot-watch`)
- Review `depot-core` API documentation
- Open an issue for questions or suggestions

