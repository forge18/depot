# GitHub Actions Workflows

This document describes all GitHub Actions workflows available in the Depot project.

## Available Workflows

### 1. CI (`ci.yml`)

**Purpose:** Continuous Integration - runs on every push and pull request

**Triggers:**
- Push to `main` or `master`
- Pull requests to `main` or `master`

**Jobs:**
- **Lint:** Checks code formatting and runs clippy
- **Test:** Runs tests on multiple platforms (Linux, macOS, Windows)
- **Security:** Runs security scans (cargo-audit, Semgrep)
- **Build:** Verifies the project builds on all platforms

**View:** Actions Tab, CI workflow

---

### 2. Release (`release.yml`)

**Purpose:** Build and publish release binaries when a version tag is pushed

**Triggers:**
- Push tag matching `v*` (e.g., `v0.1.0`)
- Manual trigger via `workflow_dispatch`

**What it does:**
1. Builds installers for all platforms (Linux, macOS, Windows)
2. Extracts release notes from CHANGELOG.md
3. Creates a GitHub release with binaries attached

**Manual Trigger:**
1. Go to Actions Tab, select the Release workflow
2. Click "Run workflow"
3. Optionally specify a version (if not using a tag)

**Example:**
```bash
# Create and push a version tag
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0

# Or trigger manually from GitHub Actions UI
```

**View:** Actions Tab, Release workflow

---

### 3. Plugin Release (`plugins-release.yml`)

**Purpose:** Build and publish plugin binaries separately from main release

**Triggers:**
- Push tag matching `depot-watch/v*` or `plugins/v*`
- Manual trigger via `workflow_dispatch`

**What it does:**
1. Builds plugin binaries for all platforms
2. Creates separate GitHub releases for each plugin
3. Uploads binaries as release assets

**Manual Trigger:**
1. Go to Actions Tab, select the Plugin Release workflow
2. Click "Run workflow"
3. Select plugin (watch, bundle, or all)
4. Enter version (e.g., v0.1.0)

**Example:**
```bash
# Release a specific plugin
git tag -a depot-watch/v0.1.0 -m "Release depot-watch v0.1.0"
git push origin depot-watch/v0.1.0

# Or release all plugins
git tag -a plugins/v0.1.0 -m "Release all plugins v0.1.0"
git push origin plugins/v0.1.0
```

**View:** Actions Tab, Plugin Release workflow

---

### 4. Wiki Sync (`wiki-sync.yml`)

**Purpose:** Automatically sync documentation from `docs/` to GitHub Wiki

**Triggers:**
- Push to `main` or `master` when files in `docs/user/**` or `docs/contributing/**` change
- Manual trigger via `workflow_dispatch`

**What it does:**
1. Syncs `docs/user/` → Wiki (User Documentation)
2. Syncs `docs/contributing/` → Wiki (Contributor Documentation)

**Manual Trigger:**
1. Go to Actions Tab, select the Sync Documentation to Wiki workflow
2. Click "Run workflow"
3. Optionally enable "Force sync"

**Requirements:**
- GitHub Personal Access Token with `repo` scope must be set as secret: `GH_PERSONAL_ACCESS_TOKEN`
- Wiki must be enabled for the repository

**Setup:**
1. Generate a Personal Access Token:
   - Go to: https://github.com/settings/tokens
   - Click "Generate new token" → "Generate new token (classic)"
   - Name: `Depot Wiki Sync`
   - Expiration: Choose your preference (or no expiration)
   - Scopes: Check `repo` (full control of private repositories)
   - Click "Generate token"
   - Copy the token immediately (you won't see it again!)

2. Add it as a repository secret:
   - Go to your repository on GitHub
   - Settings → Secrets and variables → Actions
   - Click "New repository secret"
   - Name: `GH_PERSONAL_ACCESS_TOKEN`
   - Value: Paste your token
   - Click "Add secret"

**Note:** If the token is not set, the workflow will skip wiki sync with a helpful message. This is not an error - wiki sync is optional.

**View:** Actions Tab, Wiki Sync workflow

---

## How to View Workflows

1. **GitHub Web UI:**
   - Go to your repository
   - Click "Actions" tab
   - Select the workflow from the left sidebar

2. **Workflow Status:**
   - Green checkmark = Success
   - Red X = Failed
   - Yellow circle = In progress
   - Gray circle = Cancelled

3. **Manual Triggers:**
   - Click on a workflow
   - Click "Run workflow" button (top right)
   - Fill in any required inputs
   - Click "Run workflow"

## Troubleshooting

### Workflow not showing up

- Ensure the workflow file is committed and pushed to the repository
- Check that the workflow file is in `.github/workflows/`
- Verify the YAML syntax is correct

### Workflow failing

- Check the workflow logs in the Actions tab
- Verify all required secrets are set
- Ensure permissions are correct (see workflow file)

### Manual trigger not available

- Ensure `workflow_dispatch` is in the `on:` section
- Check that you have write access to the repository
- Verify the workflow file is on the default branch

## Workflow Permissions

Each workflow specifies its required permissions:

- **CI:** No special permissions (uses default)
- **Release:** `contents: write` (to create releases)
- **Plugin Release:** `contents: write` (to create releases)
- **Wiki Sync:** `contents: write` (to update wiki)

## Best Practices

1. **Always check CI before merging PRs** - Ensures code quality
2. **Use tags for releases** - Automatic release creation
3. **Sync wiki after doc changes** - Keep documentation up to date
4. **Monitor workflow failures** - Fix issues promptly

## Related Documentation

- [Release Process](Release-Process.md) - Detailed release instructions
- [Contributing Guide](Contributing.md) - Development workflow
- [Plugin Development](Plugin-Development.md) - Plugin release process

