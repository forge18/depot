# Lua Version Manager

Depot includes a built-in Lua version manager that lets you install and switch between different Lua versions without system-wide installations.

## Overview

The Lua version manager allows you to:
- Install multiple Lua versions (5.1, 5.3, 5.4)
- Switch between versions globally or per-project
- Use project-specific versions via `.lua-version` files
- Automatically use the correct version when running scripts

## Installation Location

Lua versions are installed to:
- **Unix/macOS**: `~/.depot/versions/` (or `~/Library/Application Support/depot/versions/` on macOS)
- **Windows**: `%APPDATA%\depot\versions\`

Each version is installed in its own directory:
```
~/.depot/
├── versions/
│   ├── 5.1.5/
│   │   └── bin/
│   │       ├── lua
│   │       └── luac
│   ├── 5.3.6/
│   └── 5.4.8/
├── bin/
│   ├── lua          # Wrapper (auto-detects version)
│   └── luac         # Wrapper (auto-detects version)
└── current          # Text file with current global version
```

## Installing Lua Versions

### Install Latest Version

```bash
depot lua install latest
```

### Install Specific Version

```bash
depot lua install 5.4.8
depot lua install 5.3.6
depot lua install 5.1.5
```

### List Available Versions

```bash
depot lua list-remote
```

## Switching Versions

### Global Version

Switch the global default version:

```bash
depot lua use 5.4.8
```

This updates `~/.depot/current` and affects all projects that don't have a `.lua-version` file.

### Project-Specific Version

Set a version for the current project:

```bash
depot lua local 5.3.6
```

This creates a `.lua-version` file in your project root. The wrappers will automatically use this version when you're in this project or its subdirectories.

### Check Current Version

```bash
# Show global version
depot lua current

# Show which version will be used (respects .lua-version)
depot lua which
```

## Using Lua Versions

### Setup PATH

Add `~/.depot/bin/` to your PATH to use the `lua` and `luac` wrappers:

```bash
# Unix/macOS - add to ~/.bashrc, ~/.zshrc, etc.
export PATH="$HOME/.depot/bin:$PATH"

# Or on macOS:
export PATH="$HOME/Library/Application Support/depot/bin:$PATH"
```

### How Wrappers Work

The `lua` and `luac` wrappers in `~/.depot/bin/`:
1. Walk up the directory tree looking for `.lua-version` files
2. If found, use that version
3. Otherwise, use the global version from `~/.depot/current`
4. Execute the correct Lua binary with all arguments

### Example

```bash
# Project A uses Lua 5.3
cd project-a
depot lua local 5.3.6
lua script.lua  # Uses 5.3.6

# Project B uses Lua 5.4
cd ../project-b
depot lua local 5.4.8
lua script.lua  # Uses 5.4.8

# Outside projects, uses global version
cd ~
lua script.lua  # Uses global version (e.g., 5.4.8)
```

## Managing Versions

### List Installed Versions

```bash
depot lua list
```

Output:
```
  5.3.6
  5.4.8 (current)
```

### Uninstall a Version

```bash
depot lua uninstall 5.3.6
```

**Note**: You cannot uninstall the currently active version. Switch to another version first.

### Execute with Specific Version

Run a command with a specific version without switching:

```bash
depot lua exec 5.3.6 lua script.lua
```

## Configuration

### Custom Binary Sources

By default, Depot downloads Lua binaries from `dyne/luabinaries`. You can configure alternative sources:

```bash
# Set default source for all versions
depot config set lua_binary_source_url https://example.com/lua-binaries

# Set source for a specific version
depot config set lua_binary_sources.5.4.8 https://custom-source.com/binaries
```

### Supported Versions

Known versions with pre-built binaries:
- Lua 5.1.5
- Lua 5.3.6
- Lua 5.4.8

Future versions are supported automatically - Depot dynamically parses version numbers to determine binary names.

## Integration with Depot Scripts

When you use `depot run` or `depot exec`, Depot automatically uses the correct Lua version:
- Checks for `.lua-version` in the project
- Falls back to global version
- Uses Depot-managed Lua binaries directly (no PATH dependency)

## Troubleshooting

### "No Lua version is currently selected"

Install and use a version:
```bash
depot lua install latest
depot lua use 5.4.8
```

### "Lua binary not found"

The version might not be installed. Check with:
```bash
depot lua list
```

### Wrappers not working

Make sure `~/.depot/bin/` is in your PATH:
```bash
echo $PATH | grep depot
```

If not, add it to your shell profile.

