#!/usr/bin/env bash
set -euo pipefail

echo "=== Scenario 02: Rename Article.Body to Article.Content ==="

echo "Checking elm compilation..."
elm make src/Main.elm --output=/dev/null 2>&1
echo "PASS: Project compiles"

echo "Checking Article/Content.elm exists..."
if [ ! -f src/Article/Content.elm ]; then
    echo "FAIL: src/Article/Content.elm does not exist" >&2
    exit 1
fi
echo "PASS: Article/Content.elm exists"

echo "Checking Article/Body.elm is gone..."
if [ -f src/Article/Body.elm ]; then
    echo "FAIL: src/Article/Body.elm still exists" >&2
    exit 1
fi
echo "PASS: Article/Body.elm removed"

echo "Checking no references to Article.Body in source files..."
if grep -r "Article\.Body" src/ --include="*.elm" | grep -v "Article\.Content"; then
    echo "FAIL: References to Article.Body still exist" >&2
    exit 1
fi
echo "PASS: No remaining Article.Body references"

echo "Checking Article.Content is referenced..."
if ! grep -r "Article\.Content" src/ --include="*.elm" | grep -q .; then
    echo "FAIL: No references to Article.Content found" >&2
    exit 1
fi
echo "PASS: Article.Content is referenced"

echo "=== All checks passed ==="
