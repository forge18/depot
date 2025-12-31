# Package Management

Complete guide to managing dependencies with LPM.

## package.yaml Format

The `package.yaml` file is your project manifest:

```yaml
name: my-project
version: 1.0.0
description: "My awesome Lua project"
author: "Your Name"
license: "MIT"
lua_version: "5.4"  # Or ">=5.1", "5.1 || 5.3 || 5.4", etc.

dependencies:
  luasocket: "3.0.0"           # Exact version
  penlight: "^1.13.0"           # Compatible version
  lua-cjson: "~2.1.0"           # Patch version
  inspect: "*"                  # Any version

dev_dependencies:
  busted: "^2.0.0"              # Test framework
  luacheck: "^1.0.0"            # Linter

scripts:
  test: "busted tests/"
  lint: "luacheck src/"
  build: "lua build.lua"
```

## Version Constraints

LPM uses Semantic Versioning (SemVer) constraints with full pre-release and build metadata support:

- `"3.0.0"` - Exact version
- `"^1.13.0"` - Compatible version (>=1.13.0 <2.0.0)
- `"~2.1.0"` - Patch version (>=2.1.0 <2.2.0)
- `">=1.0.0"` - Greater than or equal
- `"<2.0.0"` - Less than
- `"1.0.0 || 2.0.0"` - Either version
- `"*"` - Any version
- `"1.0.0-alpha.1"` - Pre-release version
- `"^1.0.0-beta"` - Compatible pre-release versions

### Pre-release Versions

LPM fully supports pre-release versions following SemVer 2.0.0:

```yaml
dependencies:
  my-package: "1.0.0-beta.2"  # Exact pre-release
  other-pkg: "^1.0.0-rc"      # Compatible pre-releases (>=1.0.0-rc <2.0.0)
```

**Note**: Pre-release versions have lower precedence than normal versions. For example, `1.0.0-alpha < 1.0.0`.

## Dependency Resolution

LPM automatically resolves dependency conflicts:

1. **Version Selection**: Chooses the highest compatible version
2. **Conflict Detection**: Warns if dependencies conflict (strict mode enabled by default)
3. **Lockfile Generation**: Creates `lpm.lock` with exact versions and BLAKE3 checksums

### Example Resolution

```yaml
# package.yaml
dependencies:
  package-a: "^1.0.0"  # Needs package-b ^2.0.0
  package-b: "^1.0.0"  # Conflicts with package-a's requirement
```

LPM will detect this conflict and suggest a resolution.

### Resolution Strategies

You can configure how LPM resolves version conflicts in `package.yaml`:

```yaml
config:
  resolution_strategy: "highest"  # Default: prefer highest compatible version
  # Other options: "lowest", "exact"
```

**Available strategies**:
- `highest` (default): Selects the highest version that satisfies all constraints
- `lowest`: Selects the lowest version that satisfies all constraints (useful for testing minimum requirements)
- `exact`: Requires exact version matches (no automatic resolution)

### Strict Conflict Detection

By default, LPM uses strict conflict detection to catch potential version incompatibilities:

```yaml
config:
  strict_conflicts: true  # Default: enabled
```

When enabled, LPM will:
- Fail installation if dependency versions cannot be resolved
- Warn about transitive dependency conflicts
- Require explicit resolutions for ambiguous cases

To disable strict mode (not recommended):

```yaml
config:
  strict_conflicts: false
```

## Lockfile (lpm.lock)

The `lpm.lock` file ensures reproducible builds with cryptographic verification:

```yaml
packages:
  luasocket:
    version: "3.0.0"
    checksum: "blake3:abc123..."
    dependencies: {}
  penlight:
    version: "1.13.0"
    checksum: "blake3:def456..."
    dependencies:
      luafilesystem: "1.8.0"
```

**Important**: Commit `lpm.lock` to version control for reproducible builds.

**Checksum Algorithm**: LPM uses BLAKE3 for fast, cryptographically secure checksums to verify package integrity and prevent supply chain attacks.

## Dev Dependencies

Dev dependencies are only installed in development:

```yaml
dev_dependencies:
  busted: "^2.0.0"
  luacheck: "^1.0.0"
```

```bash
# Install all dependencies (including dev)
lpm install

# Skip dev dependencies (production)
lpm install --no-dev

# Install only dev dependencies
lpm install --dev-only
```

## Workspace Support

LPM provides monorepo/workspace support with filtering capabilities for multi-package projects.

### Workspace Structure

```
my-workspace/
├── workspace.yaml        # Workspace configuration (option 1)
├── package.yaml          # Or: root package with workspace section (option 2)
├── packages/
│   ├── package-a/
│   │   └── package.yaml
│   └── package-b/
│       └── package.yaml
└── apps/
    └── web-app/
        └── package.yaml
```

### Workspace Configuration

**Option 1: Using `workspace.yaml`**:

```yaml
name: my-workspace
packages:
  - packages/*      # Glob patterns supported
  - apps/*
  - tools/cli
```

**Option 2: Using `package.yaml` with workspace section**:

```yaml
name: my-workspace
version: 1.0.0

workspace:
  packages:
    - packages/*
    - apps/*

# Root-level shared dependencies
dependencies:
  luasocket: "^3.0.0"
```

### Package Discovery

LPM automatically discovers packages using glob patterns:

- `packages/*` - All direct subdirectories in `packages/`
- `apps/*` - All direct subdirectories in `apps/`
- `tools/cli` - Specific directory path

Packages are discovered by finding `package.yaml` files up to 3 levels deep in matching directories.

### Shared Dependencies

LPM detects dependencies shared across multiple workspace packages and can identify version conflicts:

```bash
# Check shared dependencies
lpm workspace shared-deps
```

This helps identify:
- Dependencies used by multiple packages
- Version constraint mismatches
- Opportunities for dependency hoisting

### Workspace Filtering

Filter operations to specific workspace members for faster CI/CD:

```bash
# Install dependencies only for specific packages
lpm install --filter package-a
lpm install --filter package-b

# Multiple filters
lpm install --filter package-a --filter package-b

# Glob patterns in filters
lpm install --filter "packages/*"

# Run commands on filtered workspaces
lpm run test --filter package-a
lpm build --filter package-a --filter package-b

# Include dependents (packages that depend on this package)
lpm test --filter package-a...

# Include dependencies (packages this package depends on)
lpm test --filter ...package-a
```

**Workspace filtering benefits**:
- Faster CI/CD for monorepos (only build/test changed packages)
- Selective dependency management
- Better resource utilization in large workspaces
- Support for incremental builds

**Note**: Workspace dependency inheritance and advanced features like default members are planned for future releases.

## Local Dependencies

Install packages from local paths:

```bash
lpm install --path ./local-package
```

Or in `package.yaml`:

```yaml
dependencies:
  local-pkg:
    path: "./local-package"
```

## Global Installation

Install packages globally so they're available everywhere (like `npm install -g`):

```bash
# Install globally
lpm install -g luacheck
lpm install -g busted

# Now available everywhere (after adding ~/.lpm/bin/ to PATH)
luacheck my_file.lua
busted
```

**Global installation directory:**
- Packages: `~/.lpm/global/lua_modules/`
- Executables: `~/.lpm/bin/`

**Setup PATH for global tools:**

```bash
# Unix/macOS - add to ~/.bashrc, ~/.zshrc, etc.
export PATH="$HOME/.lpm/bin:$PATH"

# Or on macOS:
export PATH="$HOME/Library/Application Support/lpm/bin:$PATH"
```

**Note**: Global tools use LPM-managed Lua versions automatically. Make sure you have a Lua version installed with `lpm lua install latest`.

## Updating Dependencies

### Update All

```bash
lpm update
```

Updates all dependencies to their latest compatible versions.

### Update Specific Package

```bash
lpm update luasocket
```

### Check for Updates

```bash
lpm outdated
```

Shows which packages have newer versions available.

## Removing Dependencies

```bash
lpm remove luasocket
```

Removes the package from `package.yaml` and `lua_modules/`.

## Verifying Dependencies

Verify package integrity:

```bash
lpm verify
```

Checks all package checksums against `lpm.lock`.

## Building from Source

LPM supports building packages from source for `make`, `cmake`, `command`, and `rust` build types:

- **`make`**: Runs `make` and `make install` to build and install native extensions
- **`cmake`**: Runs `cmake`, `cmake --build`, and `cmake --install` to build and install
- **`command`**: Runs custom build commands specified in the rockspec
- **`rust`** / **`rust-mlua`**: Builds Rust extensions using `cargo build --release` - supports packages using `luarocks-build-rust-mlua` build backend

### Prerequisites

- **For `make`**: `make` must be installed
- **For `cmake`**: `cmake` must be installed
- **For `command`**: Required build tools as specified by the package
- **For `rust`/`rust-mlua`**: Rust toolchain (`rustc`, `cargo`) must be installed

## Binary Package Support

LPM supports downloading pre-built binaries from external URLs. This is useful for packages with native extensions that don't include binaries in their source archives.

### Using Binary URLs in Rockspecs

Packages can specify binary URLs in their rockspec metadata:

```lua
-- rockspec file
metadata = {
  binary_urls = {
    ["5.4-x86_64-unknown-linux-gnu"] = "https://example.com/binary-linux-x64.so",
    ["5.4-aarch64-apple-darwin"] = "https://example.com/binary-macos-arm64.dylib",
    ["5.4-x86_64-pc-windows-msvc"] = "https://example.com/binary-windows-x64.dll",
  }
}
```

Or directly in the rockspec:

```lua
binary_urls = {
  ["5.4-x86_64-unknown-linux-gnu"] = "https://example.com/binary-linux-x64.so",
}
```

LPM will:
1. Check for a binary URL matching your Lua version and platform
2. Download the binary if available
3. Cache it for future use
4. Fall back to source installation if no binary URL is found

### Performance Benefits

- **Parallel Downloads**: LPM downloads multiple packages in parallel (up to 10 concurrent downloads) for faster installation
- **Incremental Lockfile Updates**: Only changed packages are rebuilt when updating the lockfile
- **Manifest Caching**: LuaRocks manifest is cached locally for faster lookups

