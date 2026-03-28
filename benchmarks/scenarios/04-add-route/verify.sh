#!/usr/bin/env bash
set -euo pipefail

echo "=== Scenario 04: Add Drafts Route ==="

echo "Checking elm compilation..."
elm make src/Main.elm --output=/dev/null 2>&1
echo "PASS: Project compiles"

echo "Checking Page/Drafts.elm exists..."
if [ ! -f src/Page/Drafts.elm ]; then
    echo "FAIL: src/Page/Drafts.elm does not exist" >&2
    exit 1
fi
echo "PASS: Page/Drafts.elm exists"

echo "Checking Route.elm contains Drafts..."
if ! grep -q "Drafts" src/Route.elm; then
    echo "FAIL: Drafts route not found in Route.elm" >&2
    exit 1
fi
echo "PASS: Drafts route exists"

echo "Checking Main.elm handles Drafts..."
if ! grep -q "Drafts" src/Main.elm; then
    echo "FAIL: Drafts not handled in Main.elm" >&2
    exit 1
fi
echo "PASS: Main.elm handles Drafts"

echo "Checking Page.elm has Drafts variant..."
if ! grep -q "Drafts" src/Page.elm; then
    echo "FAIL: Drafts not found in Page.elm" >&2
    exit 1
fi
echo "PASS: Page.elm has Drafts"

echo "=== All checks passed ==="
