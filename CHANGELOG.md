# Changelog

All notable changes to Depot will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- RustyHook integration for pre-commit checks
- Plugin release workflow for automated plugin builds
- Enhanced GitHub Actions workflows with better error handling
- Comprehensive documentation for releases and workflows
- `depot new` command for creating new projects in new directories
- Dynamic Lua version discovery - automatically detects installed Lua versions (5.1, 5.2, 5.3, 5.4)
- Full SemVer 2.0.0 support including pre-release versions and build metadata
- Resolution strategy configuration (highest, lowest, exact)
- Strict conflict detection mode (enabled by default)
- Workspace support for monorepos with dependency inheritance, package metadata inheritance, default members, exclude patterns, and filtering

### Changed
- Updated CodeQL Action to v4 (from deprecated v3)
- Improved wiki sync workflow to handle missing tokens gracefully
- Enhanced release workflow with manual trigger support
- Renamed lockfile from `package.lock` to `depot.lock` for clarity
- Switched to BLAKE3 checksums (from SHA-256) for faster, more secure package verification
- Code coverage thresholds: 60-70% yellow, ≥70% green (following Google Testing Blog best practices)

### Fixed
- Fixed git hooks PATH issue for RustyHook
- Fixed wiki sync workflow to not fail on missing tokens
- Fixed CI pipeline coverage measurement by excluding test files from coverage calculation

## [0.1.0] - 2024-12-06

### Added
- Initial release of Depot
- Local package installation to `./lua_modules/`
- Lockfile support (`package.lock`) for reproducible builds
- SemVer dependency resolution
- LuaRocks integration for package downloads
- Rust extension building with cross-compilation support
- Security auditing via `depot audit`
- Supply chain security with checksums
- CLI commands: init, install, remove, update, list, verify, outdated, clean
- Scripts support via `depot run` and `depot exec`
- Dev dependencies support
- Workspace/monorepo support
- Publishing to LuaRocks via `depot publish`
- Cross-platform installer generation (macOS, Linux, Windows)
- Plugin system for extensibility
- `depot-watch` plugin with enhanced features:
  - Multiple commands support (run commands in parallel)
  - Custom file type handlers (configure actions per extension: restart, reload, ignore)
  - WebSocket support for browser auto-reload
  - Enhanced terminal UI with colored output, timestamps, and status indicators
- Interactive project initialization wizard
- Interactive package installation with fuzzy search
- Project templates system (built-in and user-defined)
- Plugin management commands (`depot plugin list`, `depot plugin info`, `depot plugin update`, etc.)
- Plugin configuration system

[Unreleased]: https://github.com/yourusername/lpm/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/lpm/releases/tag/v0.1.0

