#!/usr/bin/env bash
set -euo pipefail

echo "=== Scenario 01: Add Bookmarks Feature ==="

echo "Checking elm compilation..."
elm make src/Main.elm --output=/dev/null 2>&1
echo "PASS: Project compiles"

echo "Checking Page/Bookmarks.elm exists..."
if [ ! -f src/Page/Bookmarks.elm ]; then
    echo "FAIL: src/Page/Bookmarks.elm does not exist" >&2
    exit 1
fi
echo "PASS: Page/Bookmarks.elm exists"

echo "Checking Route.elm contains Bookmarks route..."
if ! grep -q "Bookmarks" src/Route.elm; then
    echo "FAIL: Bookmarks route not found in Route.elm" >&2
    exit 1
fi
echo "PASS: Bookmarks route exists"

echo "Checking Main.elm handles Bookmarks..."
if ! grep -q "Bookmarks" src/Main.elm; then
    echo "FAIL: Bookmarks not handled in Main.elm" >&2
    exit 1
fi
echo "PASS: Main.elm handles Bookmarks"

echo "Checking navbar has Bookmarks link..."
if ! grep -q -i "bookmark" src/Page.elm; then
    echo "FAIL: Bookmarks link not found in Page.elm navbar" >&2
    exit 1
fi
echo "PASS: Navbar has Bookmarks link"

echo "=== All checks passed ==="
