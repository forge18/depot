#!/bin/bash
set -e

echo "🧪 LPM Test Suite"
echo "================="

# Parse arguments
RUN_NETWORK_TESTS=false
RUN_INTERACTIVE_TESTS=false

for arg in "$@"; do
    case $arg in
        --network)
            RUN_NETWORK_TESTS=true
            ;;
        --interactive)
            RUN_INTERACTIVE_TESTS=true
            ;;
        --all)
            RUN_NETWORK_TESTS=true
            RUN_INTERACTIVE_TESTS=true
            ;;
    esac
done

# 1. Quick unit tests
echo ""
echo "📦 Running unit tests..."
cargo test --lib

# 2. Fast integration tests
echo ""
echo "🔧 Running integration tests (no network)..."
cargo test --test integration_tests

# 3. Network tests (if enabled)
if [ "$RUN_NETWORK_TESTS" = true ]; then
    echo ""
    echo "🌐 Running network tests..."
    cargo test --test integration_tests -- --ignored --test-threads=1
else
    echo ""
    echo "⏭️  Skipping network tests (use --network to enable)"
fi

# 4. Interactive tests (if enabled)
if [ "$RUN_INTERACTIVE_TESTS" = true ]; then
    echo ""
    echo "🖥️  Running interactive tests..."
    cargo test e2e::interactive -- --ignored
else
    echo ""
    echo "⏭️  Skipping interactive tests (use --interactive to enable)"
fi

# 5. Clippy
echo ""
echo "📎 Running clippy..."
cargo clippy --all --all-features -- -D warnings

# 6. Format check
echo ""
echo "✨ Checking formatting..."
cargo fmt --all -- --check

echo ""
echo "✅ All tests passed!"
echo ""
echo "Test coverage:"
echo "  Unit tests: ✓"
echo "  Integration tests: ✓"
if [ "$RUN_NETWORK_TESTS" = true ]; then
    echo "  Network tests: ✓"
else
    echo "  Network tests: skipped"
fi
if [ "$RUN_INTERACTIVE_TESTS" = true ]; then
    echo "  Interactive tests: ✓"
else
    echo "  Interactive tests: skipped"
fi

