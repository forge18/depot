# Architecture

Overview of Depot's architecture and design decisions.

## High-Level Architecture

Depot is built in Rust and organized into several key modules:

```
┌─────────────────────────────────────────┐
│           CLI Layer (main.rs)           │
│  Parses commands, routes to handlers    │
└─────────────────────────────────────────┘
                    │
        ┌───────────┴───────────┐
        │                       │
┌───────▼────────┐    ┌────────▼────────┐
│  Package Mgmt  │    │  LuaRocks       │
│  - Install     │    │  - Client       │
│  - Update      │    │  - Parser       │
│  - Resolve     │    │  - Download     │
└───────┬────────┘    └────────┬────────┘
        │                       │
        └───────────┬───────────┘
                    │
        ┌───────────▼───────────┐
        │    Core Services      │
        │  - Error Handling     │
        │  - Path Management    │
        │  - Version Parsing    │
        │  - Cache              │
        └───────────────────────┘
```

## Core Modules

### `crates/depot-core/`

Core functionality shared across the application and plugins:

- **`core/error.rs`**: Error types and error handling
- **`core/error_help.rs`**: Contextual error messages and suggestions
- **`core/path.rs`**: Path resolution (cache, config, project paths)
- **`core/version.rs`**: Version parsing and SemVer handling
- **`core/credentials.rs`**: Secure credential storage (OS keychain)
- **`path_setup/loader.rs`**: Lua path loader generation
- **`path_setup/runner.rs`**: Lua script execution with proper paths
- **`package/manifest.rs`**: Package manifest parsing and management

The `depot-core` crate is a shared library that both the main `depot` binary and plugins can depend on. The main `depot` crate re-exports `depot-core` for backward compatibility.

### `src/cli/`

CLI command implementations:

- **`plugin.rs`**: Plugin discovery and execution
- **`install.rs`**: Package installation
- **`run.rs`**: Script execution
- **`build.rs`**: Rust extension building
- And more...

### Plugin System

Depot supports plugins as separate executables:

- **Plugin Discovery**: Automatically finds `depot-<name>` executables in `~/.depot/bin/` and PATH
- **Plugin Execution**: Delegates unknown commands to plugins
- **depot-core**: Shared library providing utilities for plugins

See [Plugin Development Guide](Plugin-Development) for details.

### `src/cli/`

CLI command implementations:

- **`init.rs`**: Initialize new project
- **`install.rs`**: Install dependencies
- **`update.rs`**: Update dependencies
- **`remove.rs`**: Remove dependencies
- **`list.rs`**: List installed packages
- **`build.rs`**: Build Rust extensions
- **`audit.rs`**: Security auditing
- And more...

### `src/package/`

Package management:

- **`manifest.rs`**: `package.yaml` parsing and management
- **`lockfile.rs`**: `depot.lock` generation and validation
- **`lockfile_builder.rs`**: Lockfile generation with parallel downloads and incremental updates
- **`resolver.rs`**: Dependency resolution
- **`installer.rs`**: Package installation to `lua_modules/`
- **`downloader.rs`**: Parallel package downloading (up to 10 concurrent downloads)
- **`checksum.rs`**: Checksum calculation and verification
- **`validator.rs`**: Manifest validation

### `src/luarocks/`

LuaRocks integration:

- **`client.rs`**: HTTP client for LuaRocks API, manifest caching
- **`rockspec.rs`**: Rockspec parsing and conversion, binary URL support
- **`rockspec_parser.rs`**: Regex-based rockspec parser, parses `binary_urls` from metadata
- **`manifest.rs`**: Rockspec manifest handling, JSON manifest parsing
- **`search_api.rs`**: Package search and version discovery via JSON manifest

### `src/resolver/`

Dependency resolution:

- **`resolver.rs`**: Main resolution algorithm
- **`dependency_graph.rs`**: Dependency graph data structure
- Handles SemVer constraints, conflict detection, version selection

### `src/build/`

Rust extension building:

- **`builder.rs`**: Main build orchestrator
- **`prebuilt.rs`**: Pre-built binary management, downloads binaries from URLs in rockspecs
- **`sandbox.rs`**: Sandboxed build environment
- **`targets.rs`**: Supported build targets

### `src/security/`

Security features:

- **`audit.rs`**: Security audit implementation
- **`advisory.rs`**: Advisory database (OSV, GitHub)
- **`vulnerability.rs`**: Vulnerability data structures

### `src/path_setup/`

Path configuration for Depot binary (not Lua paths):

- **`mod.rs`**: PATH detection and setup for Depot binary itself

Note: Lua path setup (loader and runner) has been moved to `crates/depot-core/src/path_setup/`.

## Data Flow

### Installation Flow

```
1. User: depot install luasocket
2. CLI: Parse command → install.rs
3. Resolver: Resolve dependencies → resolver.rs (uses cached manifest)
4. LuaRocks: Download packages in parallel → package/downloader.rs (ParallelDownloader)
5. Installer: Install to lua_modules/ → package/installer.rs
   - Checks for binary URLs in rockspec metadata
   - Downloads pre-built binaries if available
6. Lockfile: Update depot.lock incrementally → package/lockfile_builder.rs
```

### Build Flow

```
1. User: depot build
2. CLI: Parse command → build.rs
3. Builder: Check cache → build/builder.rs
4. Builder: Build or download pre-built → build/prebuilt.rs
5. Builder: Compile Rust → cargo-zigbuild/cargo-xwin
6. Cache: Store build artifacts → cache/cache.rs
```

## Key Design Decisions

### 1. Local Installation Only

All packages install to `./lua_modules/` - no global installation. This ensures:
- Project isolation
- Reproducible builds
- No system pollution

### 2. Lockfile-Based Reproducibility

`depot.lock` stores exact versions and BLAKE3 checksums:
- Ensures reproducible builds
- Enables checksum verification
- Prevents supply chain attacks

### 3. LuaRocks as Upstream

Depot uses LuaRocks as the package source:
- Leverages existing ecosystem
- No need to maintain separate registry
- Users can migrate easily

### 4. Rust for Performance

Depot is written in Rust for:
- Fast dependency resolution
- Safe concurrent downloads
- Cross-platform compatibility
- Strong type safety

### 5. Sandboxed Builds

Rust extensions build in sandboxed environments:
- Prevents malicious code execution
- Isolates build processes
- Protects system integrity

## Error Handling

Depot uses a custom error type (`DepotError`) with:
- Contextual error messages
- Helpful suggestions
- Chain of error causes

Example:
```rust
Err(DepotError::Package("Package not found".to_string()))
```

Automatically formatted with suggestions via `error_help.rs`.

## Caching Strategy

Depot caches:
- Downloaded packages (LuaRocks)
- Rust build artifacts (per Lua version and target)
- Pre-built binaries

Cache location: OS-specific cache directory (`~/.cache/depot` on Linux)

## Security Model

1. **Checksums**: All packages verified against lockfile
2. **No Scripts**: No postinstall script execution
3. **Sandboxed Builds**: Rust extensions build in isolation
4. **Secure Storage**: Credentials in OS keychain
5. **Audit**: Regular security audits via OSV/GitHub

## Extension Points

Depot is designed to be extensible:

- **New Commands**: Add to `src/cli/`
- **New Build Targets**: Add to `src/build/targets.rs`
- **New Resolvers**: Implement resolver trait
- **New Sources**: Implement package source trait

## Performance Considerations

- **Parallel Downloads**: Concurrent package downloads
- **Caching**: Aggressive caching of downloads and builds
- **Lazy Loading**: Load manifests only when needed
- **Incremental Builds**: Only rebuild changed components

## Testing Strategy

- **Unit Tests**: In same file as code
- **Integration Tests**: In `tests/` directory
- **Security Tests**: Specific security test suite
- **Benchmarks**: Performance benchmarks in `benches/`

