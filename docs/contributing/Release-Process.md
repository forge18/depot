# Release Process

How to create a new release of Depot.

## Pre-Release Checklist

- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Documentation is updated
- [ ] CHANGELOG is updated
- [ ] Version is bumped in `Cargo.toml`

## Version Bumping

Update version in `Cargo.toml`:

```toml
[package]
version = "0.2.0"  # Update this
```

Follow [SemVer](https://semver.org/):
- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

## Creating a Release

### 1. Update Version

```bash
# Edit Cargo.toml
vim Cargo.toml

# Commit
git add Cargo.toml
git commit -m "Bump version to 0.2.0"
```

### 2. Create Release Tag

```bash
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

### 3. GitHub Actions

The release workflow (`.github/workflows/release.yml`) automatically:
- Builds binaries for all platforms
- Creates GitHub release
- Uploads binaries as release assets

### 4. Manual Release (if needed)

If GitHub Actions fails, build manually:

```bash
# Build all installers
./scripts/build-installer.sh all

# Files will be in releases/v{VERSION}/
# Commit and push the releases/ directory
git add releases/
git commit -m "Add release files for v{VERSION}"
git push

# Create release on GitHub (via tag or workflow_dispatch)
```

## Release Assets

Each release should include:

- **macOS**: `depot-macos-aarch64.pkg`, `depot-macos-x86_64.pkg`
- **Linux**: `depot-linux-x86_64.tar.gz`, `depot-linux-aarch64.tar.gz`
- **Windows**: `depot-windows-x86_64.zip`

All follow naming: `depot-{platform}-{arch}.{ext}`

## Release Notes

Write release notes including:

- New features
- Bug fixes
- Breaking changes
- Migration guide (if needed)
- Contributors

Example:
```markdown
## v0.2.0

### Features
- Added `depot audit` command for security scanning
- Support for dev dependencies
- Workspace/monorepo support

### Bug Fixes
- Fixed PATH detection on macOS
- Resolved OpenSSL cross-compilation issues

### Breaking Changes
- None

### Contributors
- @user1
- @user2
```

## Post-Release

### 1. Update Documentation

- Update README if needed
- Update wiki if needed
- Update any version-specific docs

### 2. Announce

- Update project status
- Post to relevant communities
- Update package managers (if applicable)

### 3. Monitor

- Watch for issues
- Monitor download stats
- Check for bug reports

## Hotfix Releases

For critical bugs:

1. Create hotfix branch from latest release
2. Fix the bug
3. Bump patch version
4. Create hotfix release
5. Merge back to main

## Release Schedule

- **Major releases**: As needed (breaking changes)
- **Minor releases**: Monthly or as features accumulate
- **Patch releases**: As bugs are fixed

## Plugin Releases

Plugins (`depot-watch`) are released separately from the main Depot binary.

### Creating a Plugin Release

1. **Update plugin version** in `crates/lpm-<plugin>/Cargo.toml`

2. **Create release tag**:
   ```bash
   # For a specific plugin
   git tag -a depot-watch/v0.1.0 -m "Release depot-watch v0.1.0"
   git push origin depot-watch/v0.1.0
   
   # Or for all plugins
   git tag -a plugins/v0.1.0 -m "Release all plugins v0.1.0"
   git push origin plugins/v0.1.0
   ```

3. **GitHub Actions** automatically:
   - Builds binaries for all platforms (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64)
   - Creates GitHub releases for each plugin
   - Uploads binaries as release assets

### Plugin Release Assets

Each plugin release includes:
- **macOS**: `depot-<plugin>-macos-x86_64.tar.gz`, `depot-<plugin>-macos-aarch64.tar.gz`
- **Linux**: `depot-<plugin>-linux-x86_64.tar.gz`, `depot-<plugin>-linux-aarch64.tar.gz`
- **Windows**: `depot-<plugin>-windows-x86_64.zip`

### Plugin Release Workflow

The plugin release workflow (`.github/workflows/plugins-release.yml`) supports:
- **Tag-based releases**: Push tags like `depot-watch/v0.1.0` or `plugins/v0.1.0`
- **Manual releases**: Use GitHub Actions UI with `workflow_dispatch`

## Automation

The release process is automated via GitHub Actions:

### Main Depot Binary
- **Trigger**: Push tag `v*`
- **Build**: All platforms
- **Release**: Automatic GitHub release creation

### Plugins
- **Trigger**: Push tag `depot-<plugin>/v*` or `plugins/v*`
- **Build**: All platforms for each plugin
- **Release**: Automatic GitHub releases per plugin

Manual intervention only needed for:
- Writing release notes
- Handling build failures
- Special release requirements

