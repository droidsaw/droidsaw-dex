#!/usr/bin/env bash
# Build tests/fixtures/classes.dex from Minimal.java.
# Requires: javac, d8 (from Android SDK build-tools).
#
# Usage: ./tests/fixtures/build_fixture.sh
#   or:  ANDROID_HOME=~/android-sdk ./tests/fixtures/build_fixture.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC="$SCRIPT_DIR/java/Minimal.java"
OUT="$SCRIPT_DIR/classes.dex"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Find d8
if [ -n "${ANDROID_HOME:-}" ]; then
    D8="$(find "$ANDROID_HOME/build-tools" -name d8 -type f 2>/dev/null | sort -V | tail -1)"
else
    D8="$(command -v d8 2>/dev/null || true)"
fi
if [ -z "$D8" ]; then
    echo "error: d8 not found. Set ANDROID_HOME or add d8 to PATH." >&2
    exit 1
fi

javac -g --release 8 -d "$TMP" "$SRC"
mkdir -p "$TMP/dex"
"$D8" --no-desugaring --output "$TMP/dex" "$TMP/Minimal.class"
cp "$TMP/dex/classes.dex" "$OUT"
echo "wrote $OUT ($(wc -c < "$OUT") bytes)"
