# LPM - Lua Package Manager

**Local, project-scoped package management for Lua.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha-orange)](https://github.com/yourusername/lpm)
[![Build Status](https://github.com/forge18/lpm/actions/workflows/ci.yml/badge.svg)](https://github.com/forge18/lpm/actions/workflows/ci.yml)
[![Code Coverage](https://img.shields.io/badge/Code%20Coverage-61%25-yellow)](https://github.com/forge18/lpm/tree/main/coverage)

> **⚠️ Alpha Release Disclaimer**
> 
> LPM is currently in **alpha** status. This means:
> - The software may contain bugs and unexpected behavior
> - APIs and command-line interfaces may change without notice
> - Features may be incomplete or experimental
> - Not recommended for production use
> 
> Use at your own risk. We welcome feedback and bug reports!

LPM provides local, project-scoped package management for Lua, similar to npm, cargo, or bundler. It solves the problem of global package installations that cause dependency conflicts and make CI/CD difficult.

## Features

- **Local installation** - Dependencies install to `./lua_modules/`, not globally
- **Lua version manager** - Manage multiple Lua versions (5.1, 5.3, 5.4) with `lpm lua`
- **Global tool installation** - Install dev tools globally with `lpm install -g` (like npm)
- **Lockfile support** - Reproducible builds with `package.lock`
- **SemVer version resolution** - Proper dependency conflict resolution
- **LuaRocks compatible** - Uses LuaRocks as upstream package source
- **Rust extensions** - Build native Lua modules with Rust
- **Supply chain security** - Strong checksums, no postinstall scripts, sandboxed builds
- **Interactive CLI** - Fuzzy search, templates, and guided workflows

## Quick Start

### Installation

**Pre-built binaries (recommended):**
```bash
# macOS (Apple Silicon)
curl -L https://github.com/yourusername/lpm/releases/latest/download/lpm-v0.1.0-macos-aarch64.pkg -o lpm.pkg && open lpm.pkg

# macOS (Intel)
curl -L https://github.com/yourusername/lpm/releases/latest/download/lpm-v0.1.0-macos-x86_64.pkg -o lpm.pkg && open lpm.pkg

# Linux (x86_64)
curl -L https://github.com/yourusername/lpm/releases/latest/download/lpm-v0.1.0-linux-x86_64.tar.gz | tar xz && sudo mv lpm /usr/local/bin/

# Linux (ARM64)
curl -L https://github.com/yourusername/lpm/releases/latest/download/lpm-v0.1.0-linux-aarch64.tar.gz | tar xz && sudo mv lpm /usr/local/bin/

# Windows
# Download lpm-v0.1.0-windows-x86_64.zip from GitHub Releases
```

**From source (requires Rust):**
```bash
git clone https://github.com/yourusername/lpm.git
cd lpm
cargo build --release
cp target/release/lpm /usr/local/bin/  # or add to PATH
```

### Basic Usage

```bash
# Initialize a new project
lpm init

# Install dependencies
lpm install

# Add a package
lpm install luasocket@3.0.0

# Interactive package search
lpm install --interactive

# Run scripts
lpm run start

# Manage Lua versions
lpm lua install latest
lpm lua use 5.4.8

# Install global tools
lpm install -g lpm-watch
```

## Documentation

- **[User Guide](docs/user/Home.md)** - Complete user documentation
- **[Contributing](CONTRIBUTING.md)** - How to contribute to LPM
- **[API Documentation](docs/)** - Detailed API and architecture docs

## Plugins

LPM supports plugins that extend functionality:

- **`lpm-watch`** - Auto-reload dev server with file watching
- **`lpm-bundle`** - Bundle Lua files into a single file (experimental)

Install plugins globally: `lpm install -g lpm-watch`

## Supported Build Types

LPM supports all LuaRocks build types:
- **`builtin`/`none`** - Pure Lua modules
- **`make`** - Build from Makefile
- **`cmake`** - Build from CMakeLists.txt
- **`command`** - Custom build commands
- **`rust`/`rust-mlua`** - Rust extensions via cargo

## Requirements

- **Lua 5.1, 5.3, or 5.4** (optional - LPM includes a Lua version manager)
- **Rust** (only if building from source)

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Security

For security vulnerabilities, please see [SECURITY.md](.github/SECURITY.md).
