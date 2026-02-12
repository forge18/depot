#!/bin/bash
set -euo pipefail

# Build installer executables for Depot
# Usage: ./scripts/build-installer.sh [platform]
# Platforms: macos, windows, linux, all

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Get version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
if [ -z "$VERSION" ]; then
    echo "⚠ Failed to extract version from Cargo.toml"
    exit 1
fi

# Create versioned output directory
RELEASE_DIR="releases/v${VERSION}"
mkdir -p "$RELEASE_DIR"

PLATFORM="${1:-all}"

build_macos() {
    echo "Building macOS installer..."
    
    # Check for zig and cargo-zigbuild
    if ! command -v zig &> /dev/null; then
        echo "⚠ zig is required. Install: brew install zig"
        return 1
    fi
    
    if ! command -v cargo-zigbuild &> /dev/null; then
        echo "Installing cargo-zigbuild..."
        cargo install cargo-zigbuild 2>/dev/null || {
            echo "⚠ Failed to install cargo-zigbuild"
            return 1
        }
    fi
    
    # Set SDKROOT for macOS framework linking (required by cargo-zigbuild)
    if ! SDKROOT=$(xcrun --sdk macosx --show-sdk-path 2>/dev/null); then
        echo "⚠ Failed to find macOS SDK. Install Xcode Command Line Tools:"
        echo "   xcode-select --install"
        return 1
    fi
    export SDKROOT
    
    # Build for both architectures
    local OLD_RUSTFLAGS="${RUSTFLAGS:-}"
    for TARGET in aarch64-apple-darwin x86_64-apple-darwin; do
        if [[ "$TARGET" == "aarch64-apple-darwin" ]]; then
            ARCH="aarch64"
        else
            ARCH="x86_64"
        fi
        
        echo "Building for $TARGET ($ARCH)..."
        
        # Pass framework search path to linker via RUSTFLAGS
        export RUSTFLAGS="${OLD_RUSTFLAGS} -C link-arg=-F${SDKROOT}/System/Library/Frameworks"
        
        rustup target add "$TARGET" 2>/dev/null || true
        
        if ! cargo zigbuild --release --target "$TARGET"; then
            echo "⚠ Failed to build macOS binary for $TARGET"
            export RUSTFLAGS="${OLD_RUSTFLAGS}"
            continue
        fi
        
        export RUSTFLAGS="${OLD_RUSTFLAGS}"
        
        # Verify binary exists
        if [ ! -f "target/$TARGET/release/lpm" ]; then
            echo "⚠ Binary not found at target/$TARGET/release/lpm"
            continue
        fi
        
        # Create .pkg installer
        if command -v pkgbuild &> /dev/null; then
            mkdir -p installer-payload/usr/local/bin
            cp "target/$TARGET/release/lpm" installer-payload/usr/local/bin/
            
            OUTPUT_FILE="${RELEASE_DIR}/lpm-v${VERSION}-macos-${ARCH}.pkg"
            
            if pkgbuild --root installer-payload \
                       --identifier com.depot.installer \
                       --version "$VERSION" \
                       --install-location / \
                       "$OUTPUT_FILE"; then
                echo "✓ Created $OUTPUT_FILE"
            else
                echo "⚠ Failed to create .pkg for $ARCH"
            fi
            
            rm -rf installer-payload
        else
            echo "⚠ pkgbuild not found. Install Xcode Command Line Tools: xcode-select --install"
            # Fallback: create tar.gz instead
            mkdir -p lpm-release
            cp "target/$TARGET/release/lpm" lpm-release/
            tar czf "${RELEASE_DIR}/lpm-v${VERSION}-macos-${ARCH}.tar.gz" -C lpm-release lpm
            rm -rf lpm-release
            echo "✓ Created ${RELEASE_DIR}/lpm-v${VERSION}-macos-${ARCH}.tar.gz (fallback)"
        fi
    done
    
    # Verify at least one file was created
    if [ -z "$(ls -A ${RELEASE_DIR}/lpm-v${VERSION}-macos-* 2>/dev/null)" ]; then
        echo "⚠ No macOS installers were created"
        return 1
    fi
}

build_windows() {
    echo "Building Windows installer..."
    
    # Check for clang (required by cargo-xwin)
    if ! command -v clang &> /dev/null; then
        echo "⚠ clang is required. Install: brew install llvm"
        return 1
    fi
    
    # Check for cargo-xwin
    if ! command -v cargo-xwin &> /dev/null; then
        echo "Installing cargo-xwin..."
        cargo install cargo-xwin 2>/dev/null || {
            echo "⚠ Failed to install cargo-xwin"
            return 1
        }
    fi
    
    # Install Windows target if needed
    rustup target add x86_64-pc-windows-msvc 2>/dev/null || true
    
    # Install llvm-tools component (required for assembly dependencies)
    rustup component add llvm-tools 2>/dev/null || true
    
    # Clear any macOS-specific RUSTFLAGS that might have been set
    local OLD_RUSTFLAGS="${RUSTFLAGS:-}"
    unset RUSTFLAGS
    
    # Build Windows binary using cargo-xwin
    if ! cargo xwin build --release --target x86_64-pc-windows-msvc; then
        export RUSTFLAGS="${OLD_RUSTFLAGS}"  # Restore if it existed
        echo "⚠ Failed to build Windows binary"
        return 1
    fi
    
    export RUSTFLAGS="${OLD_RUSTFLAGS}"  # Restore if it existed
    
    if [ -f "target/x86_64-pc-windows-msvc/release/lpm.exe" ]; then
        # Create zip with executable and install script
        mkdir -p lpm-windows-release
        cp target/x86_64-pc-windows-msvc/release/lpm.exe lpm-windows-release/
        
        # Create install.bat script
        cat > lpm-windows-release/install.bat << 'EOF'
@echo off
echo Installing Depot...
set INSTALL_DIR=%USERPROFILE%\.cargo\bin
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /Y lpm.exe "%INSTALL_DIR%\lpm.exe"
echo.
echo Depot installed to %INSTALL_DIR%
echo.
echo Add to PATH (run in PowerShell as Administrator):
echo [Environment]::SetEnvironmentVariable("Path", $env:Path + ";%USERPROFILE%\.cargo\bin", "User")
echo.
echo Or manually add %USERPROFILE%\.cargo\bin to your PATH in System Properties
pause
EOF
        
        # Create zip file
        cd lpm-windows-release
        zip -q "../${RELEASE_DIR}/lpm-v${VERSION}-windows-x86_64.zip" lpm.exe install.bat
        cd ..
        rm -rf lpm-windows-release
        
        echo "✓ Created ${RELEASE_DIR}/lpm-v${VERSION}-windows-x86_64.zip"
    else
        echo "⚠ Failed to build Windows binary. Install Windows target with:"
        echo "   rustup target add x86_64-pc-windows-msvc"
        echo "   rustup toolchain install stable-x86_64-pc-windows-msvc"
    fi
}

build_linux() {
    echo "Building Linux installer..."
    
    # Check for zig and cargo-zigbuild
    if ! command -v zig &> /dev/null; then
        echo "⚠ zig is required. Install: brew install zig"
        return 1
    fi
    
    if ! command -v cargo-zigbuild &> /dev/null; then
        echo "Installing cargo-zigbuild..."
        cargo install cargo-zigbuild 2>/dev/null || {
            echo "⚠ Failed to install cargo-zigbuild"
            return 1
        }
    fi
    
    # Build for both x86_64 and aarch64 Linux using zig
    # Note: Cross-compiling to Linux from macOS requires OpenSSL handling
    # zigbuild should handle this, but if it fails, it's likely an OpenSSL issue
    for TARGET in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
        echo "Building for $TARGET..."
        rustup target add "$TARGET" 2>/dev/null || true
        
        # Clear any macOS-specific RUSTFLAGS that might have been set
        local OLD_RUSTFLAGS="${RUSTFLAGS:-}"
        unset RUSTFLAGS
        
        if ! cargo zigbuild --release --target "$TARGET"; then
            export RUSTFLAGS="${OLD_RUSTFLAGS}"  # Restore if it existed
            echo "⚠ Failed to build for $TARGET"
            continue
        fi
        
        export RUSTFLAGS="${OLD_RUSTFLAGS}"  # Restore if it existed
        
        # Create archive
        mkdir -p lpm-release
        cp "target/$TARGET/release/lpm" lpm-release/
        ARCH_NAME=$(echo "$TARGET" | sed 's/unknown-linux-gnu//' | sed 's/-//')
        tar czf "${RELEASE_DIR}/lpm-v${VERSION}-linux-${ARCH_NAME}.tar.gz" -C lpm-release lpm
        rm -rf lpm-release
        echo "✓ Created ${RELEASE_DIR}/lpm-v${VERSION}-linux-${ARCH_NAME}.tar.gz"
    done
}

case "$PLATFORM" in
    macos)
        build_macos
        ;;
    windows)
        build_windows
        ;;
    linux)
        build_linux
        ;;
    all)
        echo "Building installers for all platforms..."
        build_macos || echo "⚠ macOS build failed"
        build_linux || echo "⚠ Linux build failed"
        build_windows || echo "⚠ Windows build failed"
        echo ""
        echo ""
        echo "Build process completed. Check ${RELEASE_DIR}/ for generated installers."
        echo "Version: v${VERSION}"
        ;;
    *)
        echo "Unknown platform: $PLATFORM"
        echo "Usage: $0 [macos|windows|linux|all]"
        exit 1
        ;;
esac

