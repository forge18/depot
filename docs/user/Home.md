# Welcome to Depot Wiki

**Depot - Lua Package Manager**

Local package management for Lua. Because global installs are legacy.

## Overview

Depot provides local, project-scoped package management for Lua, similar to npm, cargo, or bundler. It solves the problem of global package installations that cause dependency conflicts and make CI/CD difficult.

## Quick Links

### User Documentation

- **[Installation Guide](Installation)** - How to install Depot
- **[Getting Started](Getting-Started)** - Your first Depot project
- **[CLI Commands](CLI-Commands)** - Complete command reference
- **[Package Management](Package-Management)** - Managing dependencies
- **[Lua Version Manager](Lua-Version-Manager)** - Managing Lua versions
- **[Rust Extensions](Rust-Extensions)** - Building native Lua modules
- **[Plugins](Plugins)** - Available plugins and usage
- **[Security](Security)** - Security features and best practices
- **[Troubleshooting](Troubleshooting)** - Common issues and solutions

### Contributor Documentation

- **[Contributing](Contributing)** - How to contribute to Depot
- **[Development Setup](Development-Setup)** - Setting up a development environment
- **[Architecture](Architecture)** - Depot architecture and design
- **[Plugin Development](Plugin-Development)** - Creating Depot plugins
- **[Testing](Testing)** - Testing guidelines
- **[Release Process](Release-Process)** - How releases are created

## Key Features

- **Local installation** - Dependencies install to `./lua_modules/`, not globally
- **Lua version manager** - Manage multiple Lua versions (5.1, 5.3, 5.4) with `depot lua`
- **Global tool installation** - Install dev tools globally with `depot install -g` (like npm)
- **Lockfile support** - Reproducible builds with `depot.lock`
- **SemVer version resolution** - Proper dependency conflict resolution
- **LuaRocks compatible** - Uses LuaRocks as upstream package source
- **Rust extensions** - Build native Lua modules with Rust, or install Rust packages from LuaRocks
- **Build from source** - Supports make, cmake, command, and rust build types
- **Supply chain security** - Strong checksums, no postinstall scripts, sandboxed builds

## Documentation

This wiki is automatically synced from the `docs/` directory in the repository. All documentation is version-controlled and updated on every commit.

