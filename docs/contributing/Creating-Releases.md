# Creating Releases with Files

This guide shows you how to create a GitHub release with all the necessary files automatically built and attached.

## Quick Start (Automatic)

The easiest way is to push a version tag - GitHub Actions will do everything:

```bash
# 1. Update version in Cargo.toml (if needed)
vim Cargo.toml  # Change version = "0.1.0" to your new version

# 2. Commit the version change
git add Cargo.toml
git commit -m "Bump version to 0.1.0"
git push

# 3. Create and push a version tag
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

**That's it!** The workflow will:
- ✅ Build binaries for all platforms (Linux, macOS, Windows)
- ✅ Extract release notes from CHANGELOG.md
- ✅ Create a GitHub release
- ✅ Attach all binary files to the release

## Method 2: Manual Trigger

If you want to trigger a release manually:

1. **Go to GitHub Actions:**
   - Navigate to your repository
   - Click the **Actions** tab
   - Select **Release** workflow from the left sidebar

2. **Run the workflow:**
   - Click **"Run workflow"** button (top right)
   - Optionally enter a version (e.g., `0.1.0`)
   - Click **"Run workflow"**

3. **Wait for completion:**
   - The workflow will build binaries
   - Create the release
   - Attach all files

## What Files Are Included?

Each release automatically includes:

### macOS
- `lpm-macos-aarch64.pkg` (Apple Silicon)
- `lpm-macos-x86_64.pkg` (Intel)

### Linux
- `lpm-linux-x86_64.tar.gz` (64-bit)
- `lpm-linux-aarch64.tar.gz` (ARM64)

### Windows
- `lpm-windows-x86_64.zip` (64-bit)

## Release Notes

The workflow automatically extracts release notes from `CHANGELOG.md`:

1. **Update CHANGELOG.md** before creating the release:
   ```markdown
   ## [0.1.0] - 2024-12-06
   
   ### Features
   - Added new feature X
   - Improved performance
   
   ### Bug Fixes
   - Fixed issue Y
   ```

2. **Commit the changelog:**
   ```bash
   git add CHANGELOG.md
   git commit -m "Update changelog for v0.1.0"
   git push
   ```

3. **Create the release tag** - the workflow will automatically use the changelog entry

## Complete Release Workflow

Here's the full process from start to finish:

```bash
# 1. Make sure everything is ready
cargo test                    # Run tests
cargo fmt --check            # Check formatting
cargo clippy                 # Check linting

# 2. Update version
vim Cargo.toml               # Update version number

# 3. Update changelog
vim CHANGELOG.md             # Add release notes

# 4. Commit changes
git add Cargo.toml CHANGELOG.md
git commit -m "Prepare release v0.1.0"
git push

# 5. Create and push tag
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0

# 6. Monitor the workflow
# Go to Actions tab and watch the release workflow run
```

## Verifying the Release

After the workflow completes:

1. **Check the release:**
   - Go to your repository
   - Click **Releases** (right sidebar)
   - Find your new release

2. **Verify files:**
   - Scroll down to "Assets"
   - You should see all platform binaries listed
   - Each file should be downloadable

3. **Check release notes:**
   - The release description should match your CHANGELOG entry
   - If no changelog entry exists, it will show a default message

## Troubleshooting

### Release not created?

- Check the Actions tab for errors
- Verify the tag format is `v*` (e.g., `v0.1.0`, not `0.1.0`)
- Ensure you have write permissions to the repository

### Files missing?

- Check the build logs in Actions
- Verify all platforms built successfully
- Look for errors in the "Build Installers" step

### Wrong version?

- The version is extracted from the tag name
- Tag format: `v0.1.0` → version `0.1.0`
- Make sure your tag matches the format

### Release notes not showing?

- Check that CHANGELOG.md has an entry matching the version
- Format: `## [0.1.0]` (brackets are important)
- The workflow will use a default message if no match is found

## Manual Release (Fallback)

If GitHub Actions fails, you can build and upload manually:

```bash
# 1. Build all installers locally
./scripts/build-installer.sh all

# 2. Files will be in .output/ directory
ls .output/

# 3. Create release on GitHub
# - Go to Releases → Draft a new release
# - Tag: v0.1.0
# - Title: Release v0.1.0
# - Description: Copy from CHANGELOG.md
# - Attach files: Upload all files from .output/
# - Publish release
```

## Best Practices

1. **Always test before releasing:**
   - Run full test suite
   - Test on multiple platforms if possible
   - Verify binaries work

2. **Update documentation:**
   - Update README if needed
   - Update any version-specific docs
   - Sync wiki (if enabled)

3. **Follow SemVer:**
   - **MAJOR** (1.0.0): Breaking changes
   - **MINOR** (0.1.0): New features
   - **PATCH** (0.0.1): Bug fixes

4. **Write good release notes:**
   - Be clear about what changed
   - Highlight breaking changes
   - Thank contributors

## Related Documentation

- [Release Process](Release-Process.md) - Detailed release procedures
- [GitHub Actions](GitHub-Actions.md) - Workflow documentation
- [Contributing Guide](Contributing.md) - Development workflow

