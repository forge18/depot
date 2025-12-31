# CLI Commands Reference

Complete reference for all LPM commands.

## Project Management

### `lpm new <name>`

Create a new LPM project in a new directory.

```bash
# Create new project
lpm new my-project

# Create with template
lpm new my-project --template love2d

# Non-interactive mode
lpm new my-project --yes
lpm new my-project -y

# Non-interactive with template
lpm new my-project --template cli-tool --yes
```

This command:
1. Creates a new directory with the specified name
2. Initializes an LPM project inside it (runs `lpm init`)
3. Sets up the project structure

**Note**: The command will fail if a directory with the same name already exists.

### `lpm init`

Initialize a new LPM project in the current directory.

```bash
# Interactive wizard mode (default)
lpm init

# Non-interactive mode (use defaults)
lpm init --yes
lpm init -y

# Use a specific template
lpm init --template <template-name>

# Non-interactive with template
lpm init --template <template-name> --yes
```

**Interactive Wizard Mode**: When run without flags, `lpm init` starts an interactive wizard that guides you through project setup:

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

**Note**: The wizard will not run if you're already in an LPM project (i.e., `package.yaml` exists in the current or parent directory).

## Dependency Management

### `lpm install [package]`

Install dependencies.

```bash
# Install all dependencies from package.yaml
lpm install

# Install a specific package
lpm install luasocket

# Install with version constraint
lpm install luasocket@3.0.0
lpm install penlight@^1.13.0

# Install as dev dependency
lpm install --dev test-more

# Install from local path
lpm install --path ./local-package

# Production install (skip dev dependencies)
lpm install --no-dev

# Install only dev dependencies
lpm install --dev-only

# Install globally (like npm install -g)
lpm install -g luacheck
lpm install -g busted

# Interactive mode: search and select packages
lpm install --interactive
lpm install -i
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

**Global Installation**: Use `-g` or `--global` to install packages globally. Global tools are installed to `~/.lpm/global/` and executables are created in `~/.lpm/bin/`. Add `~/.lpm/bin/` to your PATH to use global tools everywhere.

**Performance**: LPM downloads packages in parallel (up to 10 concurrent downloads) for faster installation. The LuaRocks manifest is cached locally to speed up dependency resolution.

### `lpm remove <package> [--global]`

Remove a dependency from your project.

```bash
# Remove from current project
lpm remove luasocket

# Remove global package
lpm remove -g luacheck
lpm remove --global busted
```

Removes the package from `package.yaml` and `lua_modules/` (or global installation directory), and deletes all associated files and executables.

### `lpm update [package]`

Update dependencies to their latest compatible versions.

```bash
# Update all dependencies
lpm update

# Update a specific package
lpm update luasocket
```

### `lpm list [--tree] [--global]`

List installed packages.

```bash
# List all packages
lpm list

# Show dependency tree
lpm list --tree

# List globally installed packages
lpm list -g
lpm list --global
```

### `lpm outdated`

Show packages that have newer versions available.

```bash
lpm outdated
```

### `lpm verify`

Verify package checksums against the lockfile.

**Note**: LPM uses BLAKE3 checksums for fast, cryptographic verification. Incremental lockfile updates mean only changed packages are rebuilt when updating `lpm.lock`, making updates faster.

```bash
lpm verify
```

## Workspace Management

### `lpm workspace list`

List all packages in the workspace.

```bash
lpm workspace list
```

Output shows:
- Workspace name and root directory
- All discovered packages with their paths
- Total package count

### `lpm workspace info`

Show detailed workspace information.

```bash
lpm workspace info
```

Displays:
- Workspace name and root directory
- Package discovery patterns
- Exclude patterns (if any)
- Default members (if specified)
- Workspace-level dependencies and dev dependencies
- Workspace package metadata (shared version, authors, license, etc.)
- All packages with their details (version, dependencies count)

### `lpm workspace shared-deps`

Analyze shared dependencies across workspace packages.

```bash
lpm workspace shared-deps
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

### `lpm run <script>`

Run a script defined in `package.yaml`.

```yaml
# package.yaml
scripts:
  test: "lua tests/run.lua"
  build: "lua build.lua"
```

```bash
lpm run test
lpm run build
```

### `lpm exec <command>`

Execute a command with correct `package.path` setup.

```bash
lpm exec lua src/main.lua
lpm exec luac -o out.luac src/main.lua
```

## Building

### `lpm build [--target <target>] [--all-targets]`

Build Rust extensions for your project.

```bash
# Build for current platform
lpm build

# Build for specific target
lpm build --target x86_64-unknown-linux-gnu

# Build for all common targets
lpm build --all-targets
```

### `lpm package [--target <target>]`

Package built binaries for distribution.

```bash
lpm package
lpm package --target x86_64-unknown-linux-gnu
```

## Publishing

### `lpm publish [--with-binaries]`

Publish your package to LuaRocks.

```bash
# Publish Lua-only package
lpm publish

# Publish with pre-built Rust binaries
lpm publish --with-binaries
```

### `lpm login`

Login to LuaRocks (stores credentials securely).

```bash
lpm login
```

### `lpm generate-rockspec`

Generate a rockspec file from `package.yaml`.

```bash
lpm generate-rockspec
```

## Maintenance

### `lpm clean`

Clean the `lua_modules/` directory.

```bash
lpm clean
```

### `lpm audit`

Run security audit on installed packages.

```bash
lpm audit
```

Checks for known vulnerabilities using OSV and GitHub Security Advisories.

## Lua Version Management

### `lpm lua install <version>`

Install a Lua version.

```bash
# Install latest version
lpm lua install latest

# Install specific version
lpm lua install 5.4.8
lpm lua install 5.3.6
lpm lua install 5.1.5
```

### `lpm lua use <version>`

Switch to a Lua version globally.

```bash
lpm lua use 5.4.8
```

### `lpm lua local <version>`

Set Lua version for current project (creates `.lua-version` file).

```bash
lpm lua local 5.3.6
```

### `lpm lua current`

Show currently active Lua version.

```bash
lpm lua current
```

### `lpm lua which`

Show which Lua version will be used (respects `.lua-version` files).

```bash
lpm lua which
```

### `lpm lua list` / `lpm lua ls`

List installed Lua versions.

```bash
lpm lua list
```

### `lpm lua list-remote` / `lpm lua ls-remote`

List available Lua versions for installation.

```bash
lpm lua list-remote
```

### `lpm lua uninstall <version>`

Uninstall a Lua version.

```bash
lpm lua uninstall 5.3.6
```

### `lpm lua exec <version> <command>`

Execute a command with a specific Lua version.

```bash
lpm lua exec 5.3.6 lua script.lua
```

**Note**: After installing Lua versions, add `~/.lpm/bin/` to your PATH to use the `lua` and `luac` wrappers. The wrappers automatically detect `.lua-version` files in your project directories.

## Plugins

LPM supports plugins that extend functionality. Plugins are automatically discovered when installed globally.

### `lpm plugin list`

List all installed plugins.

```bash
lpm plugin list
```

### `lpm plugin info <name>`

Show detailed information about a plugin.

```bash
lpm plugin info watch
```

### `lpm plugin update [name]`

Update one or all plugins to the latest version.

```bash
# Update all plugins
lpm plugin update

# Update a specific plugin
lpm plugin update watch
```

### `lpm plugin outdated`

Check for outdated plugins.

```bash
lpm plugin outdated
```

### `lpm plugin search [query]`

Search for available plugins in the registry.

```bash
# Search for plugins
lpm plugin search watch
```

### `lpm plugin config`

Manage plugin configuration.

```bash
# Get a configuration value
lpm plugin config get <plugin> <key>

# Set a configuration value
lpm plugin config set <plugin> <key> <value>

# Show all configuration for a plugin
lpm plugin config show <plugin>
```

### Plugin Commands

Once installed, plugins are available as subcommands:

```bash
# lpm-watch plugin
lpm watch [options]
lpm watch dev

# lpm-bundle plugin
lpm bundle [options]
lpm bundle watch
```

See the [Plugins documentation](Plugins.md) for detailed information about available plugins and their usage.

## Setup

### `lpm setup-path`

Automatically configure PATH for LPM (Unix/macOS only).

```bash
lpm setup-path
```

Adds `~/.cargo/bin` to your shell profile.

**For Lua version manager and global tools**, also add `~/.lpm/bin/` to your PATH:

```bash
# Unix/macOS - add to ~/.bashrc, ~/.zshrc, etc.
export PATH="$HOME/.lpm/bin:$PATH"

# Or on macOS:
export PATH="$HOME/Library/Application Support/lpm/bin:$PATH"
```

## Global Options

All commands support:

- `--version` - Show version
- `--help` - Show help for a command

```bash
lpm --version
lpm install --help
```

