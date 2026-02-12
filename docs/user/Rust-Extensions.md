# Rust Extensions

Build native Lua modules with Rust for better performance.

## Overview

Depot supports building Rust extensions that compile to native Lua modules (dynamic libraries). These modules can be loaded by Lua just like any other module.

Depot can both:
- **Build your own Rust extensions** locally using `depot build`
- **Install Rust packages from LuaRocks** that use `luarocks-build-rust-mlua` build backend

## Project Setup

Add Rust build configuration to `package.yaml`:

```yaml
name: my-project
version: 1.0.0

build:
  type: rust
  manifest: "Cargo.toml"
  modules:
    mymodule: "target/release/libmymodule.so"  # Linux/macOS
    # or
    mymodule: "target/release/mymodule.dll"    # Windows
```

## Rust Module Structure

Your Rust code should use `mlua` to create Lua modules:

```rust
// src/lib.rs
use mlua::prelude::*;

#[mlua::lua_module]
fn mymodule(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set("hello", lua.create_function(|_, ()| {
        Ok("Hello from Rust!")
    })?)?;
    Ok(exports)
}
```

```toml
# Cargo.toml
[package]
name = "mymodule"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # Important: must be cdylib

[dependencies]
mlua = { version = "0.11", features = ["lua54", "vendored"] }
```

## Installing Rust Packages from LuaRocks

Depot can install Rust packages from LuaRocks that use the `luarocks-build-rust-mlua` build backend:

```bash
# Install a Rust package from LuaRocks
depot install rustaceanvim
```

Depot will:
1. Download the package source
2. Detect it uses Rust build type
3. Run `cargo build --release` to build the extension
4. Install the built library and Lua files

**Prerequisites**: Rust toolchain (`rustc`, `cargo`) must be installed.

## Building Your Own Rust Extensions

### Build for Current Platform

```bash
depot build
```

### Build for Specific Target

```bash
depot build --target x86_64-unknown-linux-gnu
depot build --target aarch64-apple-darwin
depot build --target x86_64-pc-windows-msvc
```

### Build for All Targets

```bash
depot build --all-targets
```

Builds for all common platforms:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## Cross-Compilation

Depot uses:
- **macOS/Linux**: `cargo-zigbuild` with Zig for cross-compilation
- **Windows**: `cargo-xwin` for MSVC targets

No additional setup required - Depot handles everything automatically.

## Using Rust Modules in Lua

After building, use the module like any Lua module:

```lua
-- main.lua
local mymodule = require("mymodule")
print(mymodule.hello())  -- "Hello from Rust!"
```

Depot automatically sets up `package.cpath` to find native modules.

## Pre-built Binaries

Depot supports downloading pre-built binaries from external URLs:

1. **Checks local cache** - Uses cached binaries if available
2. **Parses binary URLs from rockspec** - Looks for `binary_urls` in rockspec metadata
3. **Downloads from URL** - Downloads binary matching your Lua version and platform
4. **Falls back to building from source** - If no binary URL is found

### Specifying Binary URLs

Add binary URLs to your rockspec metadata:

```lua
metadata = {
  binary_urls = {
    ["5.4-x86_64-unknown-linux-gnu"] = "https://github.com/user/repo/releases/download/v1.0.0/libmymodule-linux-x64.so",
    ["5.4-aarch64-apple-darwin"] = "https://github.com/user/repo/releases/download/v1.0.0/libmymodule-macos-arm64.dylib",
    ["5.4-x86_64-pc-windows-msvc"] = "https://github.com/user/repo/releases/download/v1.0.0/mymodule-windows-x64.dll",
  }
}
```

The key format is: `"{lua_version}-{target_triple}"`

Depot will automatically:
- Detect your Lua version (e.g., 5.4)
- Detect your platform (e.g., x86_64-unknown-linux-gnu)
- Match and download the appropriate binary
- Cache it for future installations

## Packaging

Package built binaries for distribution:

```bash
depot package
depot package --target x86_64-unknown-linux-gnu
```

Creates distributable archives with the compiled modules.

## Publishing

Publish packages with Rust extensions:

```bash
# Include pre-built binaries in published package
depot publish --with-binaries
```

Depot will:
1. Build for all common targets
2. Package the binaries
3. Include them in the published package

## Lua Version Support

Rust extensions must specify which Lua versions they support:

```yaml
lua_version: "5.4"  # Or ">=5.1", "5.1 || 5.3 || 5.4"
```

Depot will:
- Detect your installed Lua version
- Build extensions compatible with that version
- Cache builds per Lua version

## Best Practices

1. **Always use `cdylib`**: Rust code must compile as dynamic libraries
2. **Specify modules**: Map module names to library paths in `package.yaml`
3. **Version constraints**: Specify Lua version requirements
4. **Test locally**: Build and test before publishing
5. **Pre-built binaries**: Consider providing pre-built binaries for common targets

