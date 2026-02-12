# Git Hooks Setup

Depot uses git hooks to enforce code quality before commits.

## Quick Setup

After cloning the repository, copy the pre-commit hook to `.git/hooks/`:

```bash
cp hooks/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

The hook script is version-controlled in the `hooks/` directory so all contributors use the same checks.

## What the Hook Does

The pre-commit hook automatically runs:
- ✅ `cargo fmt --check` - Formatting check
- ✅ `cargo clippy --all-targets --all-features -- -D warnings` - Linting

The hook **prevents commits** if:
- Code is not properly formatted
- Clippy finds linting issues

Tests are run in CI, not in the pre-commit hook (for speed).

## Current Setup

### Hook Configuration

The project uses **RustyHook** to manage hook configuration via [.rusty-hook.toml](.rusty-hook.toml):

```toml
[hooks]
pre-commit = "hooks/pre-commit"
```

This tells RustyHook to execute the script at [hooks/pre-commit](hooks/pre-commit) when a commit is made.

### The Pre-Commit Script

The actual hook logic is in [hooks/pre-commit](hooks/pre-commit):

```bash
#!/bin/sh
set -e

cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings
```

### Why Both RustyHook and Manual Copy?

- **RustyHook** provides the configuration ([.rusty-hook.toml](.rusty-hook.toml)) that points to the hook script
- **Manual copy** is still required because `.git/hooks/` is not tracked by git
- The hook script in `hooks/` is version-controlled so everyone uses the same checks

### Setup for New Contributors

When setting up the repository:

1. Copy the hook to `.git/hooks/`:
   ```bash
   cp hooks/pre-commit .git/hooks/pre-commit
   chmod +x .git/hooks/pre-commit
   ```

2. The hook will now run automatically before each commit

## Modifying the Hook

To change what checks run before commits:

1. Edit [hooks/pre-commit](hooks/pre-commit)
2. Copy the updated hook to `.git/hooks/`:
   ```bash
   cp hooks/pre-commit .git/hooks/pre-commit
   ```
3. Commit the changes to `hooks/pre-commit` so other contributors get the updates

## How Git Hooks Work

Git hooks are scripts that run automatically at specific points in the git workflow:
- The `pre-commit` hook runs before a commit is created
- If the hook exits with a non-zero status, the commit is blocked
- Hook scripts must be in `.git/hooks/` and executable
- `.git/hooks/` is not tracked by git, so we store the source in `hooks/`

## Bypassing Hooks

If you need to bypass the hook (not recommended):

```bash
git commit --no-verify -m "Emergency fix"
```

⚠️ **Warning:** Only use `--no-verify` in emergencies. CI will still catch issues.

## Troubleshooting

### Hook not running

If the hook isn't executing before commits:

1. **Verify the hook was copied:**
   ```bash
   ls -la .git/hooks/pre-commit
   ```

   If missing, copy it:
   ```bash
   cp hooks/pre-commit .git/hooks/pre-commit
   ```

2. **Ensure it's executable:**
   ```bash
   chmod +x .git/hooks/pre-commit
   ```

3. **Check cargo is in PATH:**
   ```bash
   which cargo
   ```

### Hook too slow

The hook runs formatting and linting checks. If it's too slow:
- Consider using `cargo check` instead of `cargo clippy` for faster feedback
- Edit [hooks/pre-commit](hooks/pre-commit) to run only essential checks
- Remember to copy the updated hook to `.git/hooks/`

### Sharing hooks with the team

All hook-related files are version-controlled:
- [.rusty-hook.toml](.rusty-hook.toml) - RustyHook configuration
- [hooks/pre-commit](hooks/pre-commit) - Pre-commit script

When a new contributor clones the repository, they just need to:
```bash
cp hooks/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

When you update [hooks/pre-commit](hooks/pre-commit), all contributors need to re-copy it to their `.git/hooks/` directory to get the changes.

