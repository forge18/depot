# Depot - Lua Package Manager

**Local, project-scoped package management for Lua.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
![Status: Alpha](https://img.shields.io/badge/Status-Alpha-orange)

> **Alpha Release Disclaimer**
>
> Depot is currently in **alpha** status. This means:
>
> - The software may contain bugs and unexpected behavior
> - APIs and command-line interfaces may change without notice
> - Features may be incomplete or experimental
> - Not recommended for production use
>
> Use at your own risk. We welcome feedback and bug reports!

Depot provides local, project-scoped package management for Lua, similar to npm, cargo, or bundler. It solves the problem of global package installations that cause dependency conflicts and make CI/CD difficult.

## Features

- **Local installation** - Dependencies install to `./lua_modules/`, not globally
- **Lua version manager** - Manage multiple Lua versions (5.1, 5.3, 5.4) with `depot lua`
- **Global tool installation** - Install dev tools globally with `depot install -g` (like npm)
- **Lockfile support** - Reproducible builds with `depot.lock`
- **SemVer version resolution** - Proper dependency conflict resolution
- **LuaRocks compatible** - Uses LuaRocks as upstream package source
- **Rust extensions** - Build native Lua modules with Rust
- **Supply chain security** - BLAKE3 checksums, no postinstall scripts, sandboxed builds
- **Lua version compatibility** - Static analysis to detect version-specific features with `depot compat`
- **Interactive CLI** - Fuzzy search and guided workflows

## Quick Start

### Installation

**From source (requires Rust):**

```bash
cargo build --release
cp target/release/depot /usr/local/bin/  # or add to PATH
```

### Basic Usage

```bash
# Create a new project
depot new my-project

# Initialize in existing directory
depot init

# Install dependencies
depot install

# Add a package
depot install luasocket@3.0.0

# Interactive package search
depot install --interactive

# Run scripts
depot run start

# Manage Lua versions
depot lua install latest
depot lua use 5.4.8

# Install global tools
depot install -g depot-watch

# Check Lua version compatibility
depot compat
```

## Documentation

- **[User Guide](docs/user/Home.md)** - Complete user documentation
- **[Contributing](CONTRIBUTING.md)** - How to contribute to Depot
- **[API Documentation](docs/)** - Detailed API and architecture docs

## Plugins

Depot supports plugins that extend functionality:

- **`depot-watch`** - Auto-reload dev server with file watching

Install plugins globally: `depot install -g depot-watch`

## Supported Build Types

Depot supports all LuaRocks build types:

- **`builtin`/`none`** - Pure Lua modules
- **`make`** - Build from Makefile
- **`cmake`** - Build from CMakeLists.txt
- **`command`** - Custom build commands
- **`rust`/`rust-mlua`** - Rust extensions via cargo

## Requirements

- **Lua 5.1, 5.3, or 5.4** (optional - Depot includes a Lua version manager)
- **Rust** (only if building from source)

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Security

For security vulnerabilities, please see [SECURITY.md](.github/SECURITY.md).
