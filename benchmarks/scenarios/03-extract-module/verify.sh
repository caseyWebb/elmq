#!/usr/bin/env bash
set -euo pipefail

echo "=== Scenario 03: Extract Cred from Api.elm ==="

echo "Checking elm compilation..."
elm make src/Main.elm --output=/dev/null 2>&1
echo "PASS: Project compiles"

echo "Checking Api/Cred.elm exists..."
if [ ! -f src/Api/Cred.elm ]; then
    echo "FAIL: src/Api/Cred.elm does not exist" >&2
    exit 1
fi
echo "PASS: Api/Cred.elm exists"

echo "Checking Api/Cred.elm defines the Cred type..."
if ! grep -q "type Cred" src/Api/Cred.elm; then
    echo "FAIL: Cred type not defined in Api/Cred.elm" >&2
    exit 1
fi
echo "PASS: Cred type is in Api/Cred.elm"

echo "Checking Api.elm no longer defines Cred type directly..."
if grep -q "^type Cred$" src/Api.elm || grep -q "^type Cred " src/Api.elm; then
    echo "FAIL: Api.elm still defines the Cred type" >&2
    exit 1
fi
echo "PASS: Cred type removed from Api.elm"

echo "=== All checks passed ==="
