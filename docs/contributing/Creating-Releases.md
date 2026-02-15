# Creating Releases with Files

This guide shows you how to create a GitHub release with all the necessary files.

## Quick Start

The release process involves building locally, then using GitHub Actions to create the release:

```bash
# 1. Update version in Cargo.toml (if needed)
vim Cargo.toml  # Change version = "0.1.0" to your new version

# 2. Build installers for all platforms locally
./scripts/build-installer.sh all

# 3. Commit the built files
git add releases/
git commit -m "Add release files for v0.1.0"
git push

# 4. Create and push a version tag
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

**The workflow will:**
- ✅ Create a GitHub release from the tag
- ✅ Extract release notes from CHANGELOG.md
- ✅ Upload files from `releases/v{VERSION}/` to the release

## Manual Upload (After Building Locally)

If you've already built the files and pushed them, you can upload them manually:

1. **Go to GitHub Actions:**
   - Navigate to your repository
   - Click the **Actions** tab
   - Select **Release** workflow from the left sidebar

2. **Run the workflow:**
   - Click **"Run workflow"** button (top right)
   - Enter the version (e.g., `0.1.0`)
   - Click **"Run workflow"**

3. **Wait for completion:**
   - The workflow will find files in `releases/v{VERSION}/`
   - Upload them to the release

## What Files Are Included?

Each release includes versioned files in `releases/v{VERSION}/`:

### macOS
- `depot-v{VERSION}-macos-aarch64.pkg` (Apple Silicon)
- `depot-v{VERSION}-macos-x86_64.pkg` (Intel)

### Linux
- `depot-v{VERSION}-linux-x86_64.tar.gz` (64-bit)
- `depot-v{VERSION}-linux-aarch64.tar.gz` (ARM64)

### Windows
- `depot-v{VERSION}-windows-x86_64.zip` (64-bit)

**Example for v0.1.0:**
- `releases/v0.1.0/depot-v0.1.0-macos-aarch64.pkg`
- `releases/v0.1.0/depot-v0.1.0-linux-x86_64.tar.gz`
- etc.

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

# 4. Build installers locally
./scripts/build-installer.sh all

# 5. Commit changes and built files
git add Cargo.toml CHANGELOG.md releases/
git commit -m "Prepare release v0.1.0"
git push

# 6. Create and push tag
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0

# 7. Monitor the workflow
# Go to Actions tab and watch the release workflow run
```

## Verifying the Release

After the workflow completes:

1. **Check the release page:**
   - Go to your repository
   - Click **Releases** (right sidebar)
   - Find your new release (e.g., v0.1.0)

2. **Verify files are present:**
   - Scroll down to "Assets" section
   - Confirm all platform binaries are listed
   - Confirm `SHA256SUMS` file is present
   - Each file should be downloadable

3. **Test checksum verification:**

   ```bash
   # Download a binary and checksums file
   # Download a binary and checksums file from the GitHub release page
   # Verify integrity
   shasum -c SHA256SUMS
   # Expected: depot-v0.1.0-linux-x86_64.tar.gz: OK
   ```

4. **Verify release notes:**
   - Check that changelog was extracted correctly
   - Ensure all relevant changes are documented

## Troubleshooting

### Release not created?

- Check the Actions tab for errors
- Verify the tag format is `v*` (e.g., `v0.1.0`, not `0.1.0`)
- Ensure you have write permissions to the repository

### Files missing?

- Verify you built the files locally: `./scripts/build-installer.sh all`
- Check that files exist in `releases/v{VERSION}/`
- Ensure you committed and pushed the `releases/` directory
- Check the workflow logs for upload errors

### Wrong version?

- The version is extracted from the tag name
- Tag format: `v0.1.0` → version `0.1.0`
- Make sure your tag matches the format

### Release notes not showing?

- Check that CHANGELOG.md has an entry matching the version
- Format: `## [0.1.0]` (brackets are important)
- The workflow will use a default message if no match is found

## Manual Release (Fallback)

If GitHub Actions fails, you can create the release manually:

```bash
# 1. Build all installers locally
./scripts/build-installer.sh all

# 2. Files will be in releases/v{VERSION}/ directory
ls releases/v0.1.0/

# 3. Create release on GitHub
# - Go to Releases → Draft a new release
# - Tag: v0.1.0
# - Title: Release v0.1.0
# - Description: Copy from CHANGELOG.md
# - Attach files: Upload all files from releases/v0.1.0/
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

