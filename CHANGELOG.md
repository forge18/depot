# Changelog

All notable changes to LPM will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- RustyHook integration for pre-commit checks
- Plugin release workflow for automated plugin builds
- Enhanced GitHub Actions workflows with better error handling
- Comprehensive documentation for releases and workflows

### Changed
- Updated CodeQL Action to v4 (from deprecated v3)
- Improved wiki sync workflow to handle missing tokens gracefully
- Enhanced release workflow with manual trigger support

### Fixed
- Fixed git hooks PATH issue for RustyHook
- Fixed wiki sync workflow to not fail on missing tokens

## [0.1.0] - 2024-12-06

### Added
- Initial release of LPM
- Local package installation to `./lua_modules/`
- Lockfile support (`package.lock`) for reproducible builds
- SemVer dependency resolution
- LuaRocks integration for package downloads
- Rust extension building with cross-compilation support
- Security auditing via `lpm audit`
- Supply chain security with checksums
- CLI commands: init, install, remove, update, list, verify, outdated, clean
- Scripts support via `lpm run` and `lpm exec`
- Dev dependencies support
- Workspace/monorepo support
- Publishing to LuaRocks via `lpm publish`
- Cross-platform installer generation (macOS, Linux, Windows)
- Plugin system for extensibility
- `lpm-watch` plugin with enhanced features:
  - Multiple commands support (run commands in parallel)
  - Custom file type handlers (configure actions per extension: restart, reload, ignore)
  - WebSocket support for browser auto-reload
  - Enhanced terminal UI with colored output, timestamps, and status indicators
- `lpm-bundle` plugin for bundling Lua files
- Interactive project initialization wizard
- Interactive package installation with fuzzy search
- Project templates system (built-in and user-defined)
- Plugin management commands (`lpm plugin list`, `lpm plugin info`, `lpm plugin update`, etc.)
- Plugin configuration system

[Unreleased]: https://github.com/yourusername/lpm/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/lpm/releases/tag/v0.1.0

