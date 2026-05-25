#!/usr/bin/env bash
# Regenerate the LIBRARY-HELPER-RENAME R8 fixture artifacts.
#
# Inputs: source/Main.kt + source/proguard-rules.pro
# Outputs:
#   artifacts/classes.dex
#   artifacts/mapping.txt
#   artifacts/METADATA.toml
#
# This fixture is the INVERSE-CONTROL of the block_outlining fixture
# next door:
#
#   block_outlining/  -> R8 outlines, mapping carries
#                        com.android.tools.r8.outline annotations.
#   library_helper_rename/ -> R8 MINIFIES but does not outline;
#                        mapping shows class+method renames with
#                        ZERO outline annotations on MyLibraryHelpers.
#
# The mirror is intentional: the same kotlinc + R8 pipeline produces
# both, so a change to R8's defaults that breaks one is highly likely
# to be visible in the other.
#
# Requirements (must be on PATH or pinned via env vars):
#   kotlinc       — Kotlin compiler (1.9+)
#   r8            — Android R8 compiler CLI (9.0+); pin via $R8_JAR if not on PATH
#   sha256sum     — coreutils
#
# The pipeline:
#   1. kotlinc Main.kt -> a single classes.jar
#   2. r8 --release --pg-conf rules --pg-map-output mapping.txt \
#         --classfile classes.jar -> classes.dex
#   3. Record METADATA (R8 version, source sha256, outline count).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCE_DIR="$FIXTURE_ROOT/source"
ARTIFACTS_DIR="$FIXTURE_ROOT/artifacts"
TMP="$(mktemp -d -t r8libhelperfixture.XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "ERROR: $1 not on PATH. Install Android SDK build-tools or set the appropriate env var." >&2
        exit 2
    fi
}

require_tool kotlinc
require_tool sha256sum

# R8 dispatch. Invoke via explicit classpath + main class — some R8
# release jars (e.g. 9.0.32) ship with a stale `Main-Class:
# com.android.tools.r8.SwissArmyKnife` in the manifest where that
# class no longer exists, so `java -jar r8.jar` fails. Going through
# `java -cp <jar> com.android.tools.r8.R8` bypasses the manifest.
R8_CMD=""
if [[ -n "${R8_JAR:-}" ]]; then
    if [[ ! -f "$R8_JAR" ]]; then
        echo "ERROR: \$R8_JAR=$R8_JAR but the file does not exist." >&2
        exit 2
    fi
    R8_CMD="java -cp $R8_JAR com.android.tools.r8.R8"
elif command -v r8 >/dev/null 2>&1; then
    R8_CMD="r8"
else
    echo "ERROR: neither \$R8_JAR nor the 'r8' wrapper is available." >&2
    echo "  Install Android SDK build-tools (which ships r8 + d8) OR" >&2
    echo "  set R8_JAR=/path/to/r8.jar from an R8 release." >&2
    exit 2
fi

# R8 needs a library jar (Android android.jar or a JDK home) to
# resolve external references. Prefer Android SDK if available;
# otherwise fall back to the host JDK — fine for a self-contained
# Kotlin fixture that only uses stdlib (StringBuilder, println,
# Integer.toHexString/toOctalString/toBinaryString).
R8_LIB=""
if [[ -n "${ANDROID_JAR:-}" ]]; then
    R8_LIB="$ANDROID_JAR"
elif [[ -n "${ANDROID_HOME:-}" ]]; then
    R8_LIB="$(ls -d "$ANDROID_HOME"/platforms/android-*/android.jar 2>/dev/null | sort -V | tail -1)"
fi
if [[ -z "$R8_LIB" ]]; then
    R8_LIB="$(java -XshowSettings:properties -version 2>&1 | awk -F' = ' '/java.home/{print $2}' | head -1)"
fi
if [[ -z "$R8_LIB" || ! -e "$R8_LIB" ]]; then
    echo "ERROR: could not resolve R8 --lib target (looked for ANDROID_JAR, ANDROID_HOME, then JDK home)." >&2
    exit 2
fi
echo "R8 lib: $R8_LIB"

mkdir -p "$ARTIFACTS_DIR"

echo "[1/4] kotlinc -> $TMP/classes.jar"
kotlinc "$SOURCE_DIR/Main.kt" -d "$TMP/classes.jar"

# Locate the Kotlin runtime jars that kotlinc emitted references to
# (kotlin.Metadata, kotlin.jvm.JvmStatic, kotlin.jvm.internal.Intrinsics
# from kotlin-stdlib.jar; org.jetbrains.annotations.NotNull from
# annotations-*.jar). R8 9.x treats unresolved references as a hard
# error rather than a warning — they MUST be on --classpath.
KOTLINC_BIN="$(command -v kotlinc)"
KOTLIN_LIB_DIR="$(cd "$(dirname "$(readlink -f "$KOTLINC_BIN")")/../lib" && pwd)"
KOTLIN_STDLIB="$KOTLIN_LIB_DIR/kotlin-stdlib.jar"
KOTLIN_ANNOTATIONS=""
for f in "$KOTLIN_LIB_DIR"/annotations-*.jar; do
    [[ -f "$f" ]] && KOTLIN_ANNOTATIONS="$f" && break
done
for jar in "$KOTLIN_STDLIB" "$KOTLIN_ANNOTATIONS"; do
    if [[ -z "$jar" || ! -f "$jar" ]]; then
        echo "ERROR: missing kotlin runtime jar under $KOTLIN_LIB_DIR; refusing to invoke R8." >&2
        exit 2
    fi
done

echo "[2/4] r8 --release (minify + shrink, outliner enabled but expected NOT to fire)"
mkdir -p "$TMP/r8out"
$R8_CMD --release \
    --lib "$R8_LIB" \
    --classpath "$KOTLIN_STDLIB" \
    --classpath "$KOTLIN_ANNOTATIONS" \
    --pg-conf "$SOURCE_DIR/proguard-rules.pro" \
    --pg-map-output "$ARTIFACTS_DIR/mapping.txt" \
    --output "$TMP/r8out" \
    "$TMP/classes.jar"

# r8 emits classes.dex in $TMP/r8out.
if [[ ! -f "$TMP/r8out/classes.dex" ]]; then
    echo "ERROR: r8 did not emit classes.dex (look in $TMP/r8out)." >&2
    exit 3
fi
cp "$TMP/r8out/classes.dex" "$ARTIFACTS_DIR/classes.dex"

echo "[3/4] sanity-check: outline annotations on MyLibraryHelpers"
# This fixture's whole point is that the outliner DOES NOT fire on
# MyLibraryHelpers. We still record any outline annotations the
# mapping carries (e.g. from kotlin-stdlib if R8 outlines something
# in there); the ratchet test asserts on the helper-class-specific
# slice rather than the global count.
OUTLINE_COUNT="$(grep -c 'com.android.tools.r8.outline"' "$ARTIFACTS_DIR/mapping.txt" || true)"
HELPER_OUTLINE_COUNT="$(grep -c 'MyLibraryHelpers.*com.android.tools.r8.outline' "$ARTIFACTS_DIR/mapping.txt" || true)"
echo "  global outline annotations: $OUTLINE_COUNT"
echo "  helper-class outline annotations: $HELPER_OUTLINE_COUNT (expected: 0)"
if [[ "$HELPER_OUTLINE_COUNT" -gt 0 ]]; then
    echo "WARN: mapping shows com.android.tools.r8.outline on MyLibraryHelpers." >&2
    echo "      The Kotlin bodies have collapsed into something R8's outliner" >&2
    echo "      can extract. Re-tune source/Main.kt so the helper bodies stay" >&2
    echo "      structurally distinct." >&2
fi

echo "[4/4] METADATA.toml"
R8_VERSION="$($R8_CMD --version 2>&1 | head -1 || true)"
[[ -n "$R8_VERSION" ]] || R8_VERSION="unknown"
SOURCE_SHA="$(sha256sum "$SOURCE_DIR/Main.kt" | awk '{print $1}')"
RULES_SHA="$(sha256sum "$SOURCE_DIR/proguard-rules.pro" | awk '{print $1}')"
DEX_SHA="$(sha256sum "$ARTIFACTS_DIR/classes.dex" | awk '{print $1}')"
MAP_SHA="$(sha256sum "$ARTIFACTS_DIR/mapping.txt" | awk '{print $1}')"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat >"$ARTIFACTS_DIR/METADATA.toml" <<EOF
# Auto-generated by tests/fixtures/r8/library_helper_rename/scripts/regen-library-helper-fixture.sh.
# Do not edit by hand; re-run the script after touching source/.

r8_version = "$R8_VERSION"
generated_at = "$TIMESTAMP"
# Global count across the whole DEX (kotlin-stdlib transitive may add some).
outline_annotation_count = $OUTLINE_COUNT
# Count specifically on the engineered MyLibraryHelpers slice — MUST be 0.
helper_outline_annotation_count = $HELPER_OUTLINE_COUNT

[source]
main_kt_sha256 = "$SOURCE_SHA"
proguard_rules_sha256 = "$RULES_SHA"

[artifacts]
classes_dex_sha256 = "$DEX_SHA"
mapping_txt_sha256 = "$MAP_SHA"
EOF

echo "Fixture regenerated."
echo "  classes.dex : $ARTIFACTS_DIR/classes.dex ($DEX_SHA)"
echo "  mapping.txt : $ARTIFACTS_DIR/mapping.txt ($MAP_SHA, $OUTLINE_COUNT global / $HELPER_OUTLINE_COUNT helper outline annotations)"
echo "  METADATA    : $ARTIFACTS_DIR/METADATA.toml"
