#!/usr/bin/env bash
# Regenerate test fixture DEX files from Java sources.
# Requires: javac (via JAVA_HOME or PATH), d8 (via ANDROID_HOME or PATH).
# Run from the repo root or tests/ directory.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FIXTURES="$SCRIPT_DIR/fixtures"
JAVA_DIR="$FIXTURES/java"

# Resolve javac
if [[ -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/javac" ]]; then
    JAVAC="$JAVA_HOME/bin/javac"
else
    JAVAC="$(command -v javac)"
fi

# Resolve d8
D8=""
if [[ -n "${ANDROID_HOME:-}" ]]; then
    D8="$(find "$ANDROID_HOME/build-tools" -name d8 -type f 2>/dev/null | sort -r | head -1)"
fi
if [[ -z "$D8" ]]; then
    D8="$(command -v d8 2>/dev/null || true)"
fi

if [[ -z "$JAVAC" ]]; then echo "ERROR: javac not found"; exit 1; fi
if [[ -z "$D8" ]];   then echo "ERROR: d8 not found (set ANDROID_HOME)"; exit 1; fi

echo "javac: $JAVAC"
echo "d8:    $D8"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# ── classes.dex (Minimal) ───────────────────────────────────────────────────
# Used by unit tests via include_bytes!("../tests/fixtures/classes.dex").
# Built with `-g -parameters` so LocalVariableTable + MethodParameters
# survive into the DEX. The legacy `debug::tests::parse_fixture_debug_info`
# asserts the ctor's param `x` appears in the debug-info name stream;
# default-flag `javac --release 8` on Fedora-packaged JDK 21 drops local
# names, making the test flake on fresh regen. Tracked as a known flake.
echo "Building classes.dex from Minimal.java..."
"$JAVAC" --release 8 -g -parameters -Xlint:none -d "$WORK" "$JAVA_DIR/Minimal.java"
mkdir -p "$WORK/minimal_dex"
"$D8" --no-desugaring --output "$WORK/minimal_dex" "$WORK/Minimal.class"
cp "$WORK/minimal_dex/classes.dex" "$FIXTURES/classes.dex"
echo "  -> fixtures/classes.dex"

# ── classes_named.dex (MinimalNamed) ───────────────────────────────────────
# Used by unit tests exercising debug_info local-variable name propagation.
# Built with `-g -parameters` so LocalVariableTable + MethodParameters survive
# into the DEX; without these flags d8 strips local names and the name-
# propagation loop has nothing to bind.
echo "Building classes_named.dex from MinimalNamed.java..."
"$JAVAC" --release 8 -g -parameters -Xlint:none -d "$WORK" "$JAVA_DIR/MinimalNamed.java"
mkdir -p "$WORK/minimal_named_dex"
"$D8" --no-desugaring --output "$WORK/minimal_named_dex" "$WORK/MinimalNamed.class"
cp "$WORK/minimal_named_dex/classes.dex" "$FIXTURES/classes_named.dex"
echo "  -> fixtures/classes_named.dex"

# ── round-trip fixtures ─────────────────────────────────────────────────────
# The dex_roundtrip integration test builds these on the fly, so we don't need
# pre-built DEX here.  This block is intentionally left empty but left as a
# hook for fixtures that require pre-built DEX (e.g. multi-class DEX files).

echo "Done."
