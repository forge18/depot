# CLI Commands Reference

Complete reference for all Depot commands.

## Project Management

### `depot new <name>`

Create a new Depot project in a new directory.

```bash
# Create new project
depot new my-project

# Create with template
depot new my-project --template love2d

# Non-interactive mode
depot new my-project --yes
depot new my-project -y

# Non-interactive with template
depot new my-project --template cli-tool --yes
```

This command:
1. Creates a new directory with the specified name
2. Initializes an Depot project inside it (runs `depot init`)
3. Sets up the project structure

**Note**: The command will fail if a directory with the same name already exists.

### `depot init`

Initialize a new Depot project in the current directory.

```bash
# Interactive wizard mode (default)
depot init

# Non-interactive mode (use defaults)
depot init --yes
depot init -y

# Use a specific template
depot init --template <template-name>

# Non-interactive with template
depot init --template <template-name> --yes
```

**Interactive Wizard Mode**: When run without flags, `depot init` starts an interactive wizard that guides you through project setup:

1. **Project name**: Enter a name for your project (defaults to current directory name)
   - Must contain only alphanumeric characters, hyphens, and underscores
   - Validates input before proceeding

2. **Project version**: Enter the initial version (default: `1.0.0`)
   - Follows semantic versioning (e.g., `1.0.0`, `0.1.0`)

3. **Description**: Optional project description
   - Can be left empty

4. **License**: Select from common licenses:
   - MIT (default)
   - Apache-2.0
   - BSD-3-Clause
   - GPL-3.0
   - LGPL-3.0
   - ISC
   - Unlicense
   - None

5. **Lua version**: Select the Lua version requirement:
   - 5.1
   - 5.3
   - 5.4 (default)
   - latest

6. **Template selection**: Optionally select a project template:
   - None (empty project)
   - basic-lua - Basic Lua project structure
   - love2d - Love2D game development template
   - neovim-plugin - Neovim plugin template
   - lapis-web - OpenResty/Lapis web application template
   - cli-tool - CLI tool template
   - Any custom templates you've created

7. **Summary and confirmation**: Review all selections before creating the project

**Non-Interactive Mode**: Use `--yes` or `-y` to skip the wizard and use default values:
- Project name: Current directory name
- Version: `1.0.0`
- Description: None
- License: None
- Lua version: `5.4`
- Template: None (unless `--template` is specified)

**Template Usage**: Use `--template <name>` to directly specify a template:
- Works in both interactive and non-interactive modes
- In interactive mode, skips template selection step
- In non-interactive mode, applies template with default variables

**What Gets Created**:
- `package.yaml` - Project manifest with all configuration
- Directory structure:
  - `src/` - Source code directory
  - `lib/` - Library code directory
  - `tests/` - Test files directory
- Template files (if template selected)
- Basic `src/main.lua` (if no template used)

**Note**: The wizard will not run if you're already in an Depot project (i.e., `package.yaml` exists in the current or parent directory).

## Dependency Management

### `depot install [package]`

Install dependencies.

```bash
# Install all dependencies from package.yaml
depot install

# Install a specific package
depot install luasocket

# Install with version constraint
depot install luasocket@3.0.0
depot install penlight@^1.13.0

# Install as dev dependency
depot install --dev test-more

# Install from local path
depot install --path ./local-package

# Production install (skip dev dependencies)
depot install --no-dev

# Install only dev dependencies
depot install --dev-only

# Install globally (like npm install -g)
depot install -g luacheck
depot install -g busted

# Interactive mode: search and select packages
depot install --interactive
depot install -i
```

**Interactive Mode**: Use `-i` or `--interactive` to search and install packages interactively. This mode provides:
- **Fuzzy search**: Search for packages by name with intelligent matching
- **Version selection**: Choose from all available versions for each package
- **Dependency type selection**: Choose whether each package is a production or development dependency
- **Batch selection**: Select multiple packages at once
- **Installation summary**: Review all selections before installing

**Interactive Flow**:
1. Enter a search query to find packages
2. Select one or more packages from the search results
3. For each selected package:
   - Choose a version from the available versions (latest is selected by default)
   - Choose dependency type (production or development)
4. Review the installation summary
5. Confirm to install

**Global Installation**: Use `-g` or `--global` to install packages globally. Global tools are installed to `~/.depot/global/` and executables are created in `~/.depot/bin/`. Add `~/.depot/bin/` to your PATH to use global tools everywhere.

**Performance**: Depot downloads packages in parallel (up to 10 concurrent downloads) for faster installation. The LuaRocks manifest is cached locally to speed up dependency resolution.

### `depot remove <package> [--global]`

Remove a dependency from your project.

```bash
# Remove from current project
depot remove luasocket

# Remove global package
depot remove -g luacheck
depot remove --global busted
```

Removes the package from `package.yaml` and `lua_modules/` (or global installation directory), and deletes all associated files and executables.

### `depot update [package]`

Update dependencies to their latest compatible versions.

```bash
# Update all dependencies
depot update

# Update a specific package
depot update luasocket
```

### `depot list [--tree] [--global]`

List installed packages.

```bash
# List all packages
depot list

# Show dependency tree
depot list --tree

# List globally installed packages
depot list -g
depot list --global
```

### `depot outdated`

Show packages that have newer versions available.

```bash
depot outdated
```

### `depot verify`

Verify package checksums against the lockfile.

**Note**: Depot uses BLAKE3 checksums for fast, cryptographic verification. Incremental lockfile updates mean only changed packages are rebuilt when updating `depot.lock`, making updates faster.

```bash
depot verify
```

## Workspace Management

### `depot workspace list`

List all packages in the workspace.

```bash
depot workspace list
```

Output shows:
- Workspace name and root directory
- All discovered packages with their paths
- Total package count

### `depot workspace info`

Show detailed workspace information.

```bash
depot workspace info
```

Displays:
- Workspace name and root directory
- Package discovery patterns
- Exclude patterns (if any)
- Default members (if specified)
- Workspace-level dependencies and dev dependencies
- Workspace package metadata (shared version, authors, license, etc.)
- All packages with their details (version, dependencies count)

### `depot workspace shared-deps`

Analyze shared dependencies across workspace packages.

```bash
depot workspace shared-deps
```

Shows:
- Dependencies used by multiple packages
- Version constraints for each usage
- **Warning indicators** for version conflicts

This helps identify:
- Opportunities for dependency hoisting to workspace level
- Version mismatches that should be unified
- Packages with inconsistent dependency versions

## Scripts and Execution

### `depot run <script>`

Run a script defined in `package.yaml`.

```yaml
# package.yaml
scripts:
  test: "lua tests/run.lua"
  build: "lua build.lua"
```

```bash
depot run test
depot run build
```

### `depot exec <command>`

Execute a command with correct `package.path` setup.

```bash
depot exec lua src/main.lua
depot exec luac -o out.luac src/main.lua
```

## Building

### `depot build [--target <target>] [--all-targets]`

Build Rust extensions for your project.

```bash
# Build for current platform
depot build

# Build for specific target
depot build --target x86_64-unknown-linux-gnu

# Build for all common targets
depot build --all-targets
```

### `depot package [--target <target>]`

Package built binaries for distribution.

```bash
depot package
depot package --target x86_64-unknown-linux-gnu
```

## Publishing

### `depot publish [--with-binaries]`

Publish your package to LuaRocks.

```bash
# Publish Lua-only package
depot publish

# Publish with pre-built Rust binaries
depot publish --with-binaries
```

### `depot login`

Login to LuaRocks (stores credentials securely).

```bash
depot login
```

### `depot generate-rockspec`

Generate a rockspec file from `package.yaml`.

```bash
depot generate-rockspec
```

## Maintenance

### `depot clean`

Clean the `lua_modules/` directory.

```bash
depot clean
```

### `depot audit`

Run security audit on installed packages.

```bash
depot audit
```

Checks for known vulnerabilities using OSV and GitHub Security Advisories.

## Lua Version Management

### `depot lua install <version>`

Install a Lua version.

```bash
# Install latest version
depot lua install latest

# Install specific version
depot lua install 5.4.8
depot lua install 5.3.6
depot lua install 5.1.5
```

### `depot lua use <version>`

Switch to a Lua version globally.

```bash
depot lua use 5.4.8
```

### `depot lua local <version>`

Set Lua version for current project (creates `.lua-version` file).

```bash
depot lua local 5.3.6
```

### `depot lua current`

Show currently active Lua version.

```bash
depot lua current
```

### `depot lua which`

Show which Lua version will be used (respects `.lua-version` files).

```bash
depot lua which
```

### `depot lua list` / `depot lua ls`

List installed Lua versions.

```bash
depot lua list
```

### `depot lua list-remote` / `depot lua ls-remote`

List available Lua versions for installation.

```bash
depot lua list-remote
```

### `depot lua uninstall <version>`

Uninstall a Lua version.

```bash
depot lua uninstall 5.3.6
```

### `depot lua exec <version> <command>`

Execute a command with a specific Lua version.

```bash
depot lua exec 5.3.6 lua script.lua
```

**Note**: After installing Lua versions, add `~/.depot/bin/` to your PATH to use the `lua` and `luac` wrappers. The wrappers automatically detect `.lua-version` files in your project directories.

## Plugins

Depot supports plugins that extend functionality. Plugins are automatically discovered when installed globally.

### `depot plugin list`

List all installed plugins.

```bash
depot plugin list
```

### `depot plugin info <name>`

Show detailed information about a plugin.

```bash
depot plugin info watch
```

### `depot plugin update [name]`

Update one or all plugins to the latest version.

```bash
# Update all plugins
depot plugin update

# Update a specific plugin
depot plugin update watch
```

### `depot plugin outdated`

Check for outdated plugins.

```bash
depot plugin outdated
```

### `depot plugin search [query]`

Search for available plugins in the registry.

```bash
# Search for plugins
depot plugin search watch
```

### `depot plugin config`

Manage plugin configuration.

```bash
# Get a configuration value
depot plugin config get <plugin> <key>

# Set a configuration value
depot plugin config set <plugin> <key> <value>

# Show all configuration for a plugin
depot plugin config show <plugin>
```

### Plugin Commands

Once installed, plugins are available as subcommands:

```bash
# depot-watch plugin
depot watch [options]
depot watch dev
```

See the [Plugins documentation](Plugins.md) for detailed information about available plugins and their usage.

## Setup

### `depot setup-path`

Automatically configure PATH for Depot (Unix/macOS only).

```bash
depot setup-path
```

Adds `~/.cargo/bin` to your shell profile.

**For Lua version manager and global tools**, also add `~/.depot/bin/` to your PATH:

```bash
# Unix/macOS - add to ~/.bashrc, ~/.zshrc, etc.
export PATH="$HOME/.depot/bin:$PATH"

# Or on macOS:
export PATH="$HOME/Library/Application Support/lpm/bin:$PATH"
```

## Global Options

All commands support:

- `--version` - Show version
- `--help` - Show help for a command

```bash
depot --version
depot install --help
```

