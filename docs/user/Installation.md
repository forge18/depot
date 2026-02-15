# Installation Guide

## Prerequisites

- Lua 5.1, 5.3, or 5.4 installed

## Installation Methods

### Option 1: Build Locally (Requires Rust)

If you have Rust installed ([rustup.rs](https://rustup.rs/)):

```bash
# Build the release executable
cargo build --release

# The executable will be at: target/release/depot (or target/release/depot.exe on Windows)
# Copy it wherever you want:
cp target/release/depot /usr/local/bin/depot  # Unix/macOS
# Or on Windows, add target/release/ to your PATH
```

### Option 2: Install via Cargo (Requires Rust)

```bash
# Install from crates.io (when published)
cargo install depot

# Or install from local source
cargo install --path .
```

## Setup PATH

After installation, ensure `depot` is in your PATH:

### Unix/macOS/Linux

Run the setup command:
```bash
depot setup
```

Or manually add to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.):
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### Windows

Add `%USERPROFILE%\.cargo\bin` to your PATH:
- Open System Properties → Environment Variables
- Add `%USERPROFILE%\.cargo\bin` to your User PATH
- Or run in PowerShell (as Administrator):
```powershell
[Environment]::SetEnvironmentVariable("Path", $env:Path + ";%USERPROFILE%\.cargo\bin", "User")
```

## Verify Installation

Check that Depot is installed correctly:

```bash
depot --version
```

You should see the version number. If you get a "command not found" error, make sure:
1. Depot is installed
2. The installation directory is in your PATH
3. You've restarted your terminal (or reloaded your shell profile)

