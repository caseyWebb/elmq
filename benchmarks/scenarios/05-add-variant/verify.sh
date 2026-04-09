#!/usr/bin/env bash
set -euo pipefail

echo "=== Scenario 05: Add BookmarkedFeed Variant ==="

echo "Checking elm compilation..."
elm make src/Main.elm --output=/dev/null 2>&1
echo "PASS: Project compiles"

echo "Checking FeedTab has BookmarkedFeed variant..."
if ! grep -q "BookmarkedFeed" src/Page/Home.elm; then
    echo "FAIL: BookmarkedFeed not found in Page/Home.elm" >&2
    exit 1
fi
echo "PASS: BookmarkedFeed variant exists"

echo "Checking BookmarkedFeed is handled in case expressions..."
CASE_COUNT=$(grep -c "BookmarkedFeed" src/Page/Home.elm || true)
if [ "$CASE_COUNT" -lt 3 ]; then
    echo "FAIL: BookmarkedFeed appears only $CASE_COUNT times (expected at least 3 — type + case branches)" >&2
    exit 1
fi
echo "PASS: BookmarkedFeed handled in $CASE_COUNT locations"

echo "=== All checks passed ==="
