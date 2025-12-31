#!/bin/bash

# Calculate code coverage percentage from lcov.info and update README badge

LCOV_FILE="coverage/lcov.info"
README_FILE="README.md"

if [ ! -f "$LCOV_FILE" ]; then
    echo "Error: $LCOV_FILE not found"
    exit 1
fi

# Parse lcov.info to calculate coverage percentage
# Count lines hit and lines found
LINES_HIT=$(grep "^LH:" "$LCOV_FILE" | awk -F: '{sum += $2} END {print sum}')
LINES_FOUND=$(grep "^LF:" "$LCOV_FILE" | awk -F: '{sum += $2} END {print sum}')

if [ -z "$LINES_HIT" ] || [ -z "$LINES_FOUND" ] || [ "$LINES_FOUND" -eq 0 ]; then
    echo "Error: Could not parse coverage data from $LCOV_FILE"
    exit 1
fi

# Calculate percentage
COVERAGE=$((LINES_HIT * 100 / LINES_FOUND))

echo "Code Coverage: $COVERAGE% ($LINES_HIT / $LINES_FOUND lines)"

# Determine badge color based on coverage
if [ "$COVERAGE" -ge 70 ]; then
    COLOR="brightgreen"
elif [ "$COVERAGE" -ge 60 ]; then
    COLOR="yellow"
else
    COLOR="red"
fi

# Create badge URL
BADGE_URL="https://img.shields.io/badge/Code%20Coverage-${COVERAGE}%25-${COLOR}"
NEW_BADGE="[![Code Coverage]($BADGE_URL)](https://github.com/forge18/lpm/tree/main/coverage)"

# Update README with new badge using a regex pattern that matches any coverage badge
# This handles both "Coming Soon" and previously generated percentage badges
if grep -q "Code Coverage" "$README_FILE"; then
    # Use perl for cross-platform regex replacement (works on both macOS and Linux)
    perl -i -pe 's|\[!\[Code Coverage\]\(https://img\.shields\.io/badge/Code%20Coverage-[^)]+\)\]\(https://github\.com/forge18/lpm/tree/main/coverage\)|'"$NEW_BADGE"'|g' "$README_FILE"
    echo "✓ Updated coverage badge in $README_FILE"
else
    echo "Error: Could not find coverage badge in $README_FILE"
    exit 1
fi
