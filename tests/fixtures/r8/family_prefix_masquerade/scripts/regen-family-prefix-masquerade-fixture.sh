#!/usr/bin/env bash
# Regenerate the FAMILY-PREFIX-MASQUERADE adversarial PoC fixture.
#
# Inputs: source/Main.kt + source/proguard-rules.pro
# Outputs:
#   artifacts/classes.dex
#   artifacts/mapping.txt
#   artifacts/METADATA.toml
#
# Threat model documented on KNOWN_FP_FAMILY in
# tests/r8_fdroid_apk_sweep.rs. This fixture demonstrates the
# masquerade end-to-end: a class FQCN starting with `androidx.`
# satisfies the recogniser's I4-I13 invariants without being a real
# R8 outline, so mapping-less family suppression hides it.
#
# Requirements (must be on PATH or pinned via env vars):
#   kotlinc       — Kotlin compiler (1.9+)
#   r8            — Android R8 compiler CLI (9.0+); pin via $R8_JAR if not on PATH
#   sha256sum     — coreutils

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCE_DIR="$FIXTURE_ROOT/source"
ARTIFACTS_DIR="$FIXTURE_ROOT/artifacts"
TMP="$(mktemp -d -t r8masqfixture.XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "ERROR: $1 not on PATH. Install Android SDK build-tools or set the appropriate env var." >&2
        exit 2
    fi
}

require_tool kotlinc
require_tool sha256sum

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
    exit 2
fi

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
    echo "ERROR: could not resolve R8 --lib target." >&2
    exit 2
fi
echo "R8 lib: $R8_LIB"

mkdir -p "$ARTIFACTS_DIR"

echo "[1/4] kotlinc -> $TMP/classes.jar"
kotlinc "$SOURCE_DIR/Main.kt" -d "$TMP/classes.jar"

KOTLINC_BIN="$(command -v kotlinc)"
KOTLIN_LIB_DIR="$(cd "$(dirname "$(readlink -f "$KOTLINC_BIN")")/../lib" && pwd)"
KOTLIN_STDLIB="$KOTLIN_LIB_DIR/kotlin-stdlib.jar"
KOTLIN_ANNOTATIONS=""
for f in "$KOTLIN_LIB_DIR"/annotations-*.jar; do
    [[ -f "$f" ]] && KOTLIN_ANNOTATIONS="$f" && break
done
for jar in "$KOTLIN_STDLIB" "$KOTLIN_ANNOTATIONS"; do
    if [[ -z "$jar" || ! -f "$jar" ]]; then
        echo "ERROR: missing kotlin runtime jar under $KOTLIN_LIB_DIR." >&2
        exit 2
    fi
done

echo "[2/4] r8 --release --no-minification --no-tree-shaking (preserve class names + caller methods)"
# --no-minification is the load-bearing flag: it disables R8's name
# obfuscator, keeping androidx.adversarial.poc.* class names verbatim
# in the DEX (the masquerade premise). pg-conf `-dontobfuscate` alone
# is ignored under --release (R8 9.x ignores some pg-conf options in
# release mode); the CLI flag is the only reliable mechanism.
# --no-tree-shaking prevents R8 from inlining the 25 single-call
# `callNN` methods into Main.main, which would dissolve the
# distinct-caller-count gate the recogniser needs.
mkdir -p "$TMP/r8out"
$R8_CMD --release \
    --no-minification \
    --no-tree-shaking \
    --lib "$R8_LIB" \
    --classpath "$KOTLIN_STDLIB" \
    --classpath "$KOTLIN_ANNOTATIONS" \
    --pg-conf "$SOURCE_DIR/proguard-rules.pro" \
    --pg-map-output "$ARTIFACTS_DIR/mapping.txt" \
    --output "$TMP/r8out" \
    "$TMP/classes.jar"

if [[ ! -f "$TMP/r8out/classes.dex" ]]; then
    echo "ERROR: r8 did not emit classes.dex (look in $TMP/r8out)." >&2
    exit 3
fi
cp "$TMP/r8out/classes.dex" "$ARTIFACTS_DIR/classes.dex"

echo "[3/4] sanity-check: outline annotation count on Backdoor"
# Whole-point assertion: the masquerade succeeds because Backdoor is
# NOT a real R8 outline. The mapping must carry ZERO outline
# annotations on the androidx.adversarial.poc.* slice — any non-zero
# count means R8 actually outlined our helpers, which would defeat
# the demonstration. Re-tune source/Main.kt or proguard-rules.pro to
# prevent outlining if this trips.
OUTLINE_COUNT="$(grep -c 'com.android.tools.r8.outline"' "$ARTIFACTS_DIR/mapping.txt" || true)"
ADVERSARIAL_OUTLINE_COUNT="$(grep -c 'androidx\.adversarial\.poc.*com.android.tools.r8.outline' "$ARTIFACTS_DIR/mapping.txt" || true)"
echo "  global outline annotations: $OUTLINE_COUNT"
echo "  androidx.adversarial.poc outline annotations: $ADVERSARIAL_OUTLINE_COUNT (expected: 0)"
if [[ "$ADVERSARIAL_OUTLINE_COUNT" -gt 0 ]]; then
    echo "ERROR: R8 outlined the adversarial slice — masquerade demo broken." >&2
    echo "      Re-tune source/Main.kt body or proguard-rules.pro to prevent outlining." >&2
    exit 3
fi

# Verify the class FQCN actually starts with androidx. in the
# mapping. If the kotlinc package declaration somehow drifted, the
# masquerade premise fails.
PACKAGE_HITS="$(grep -c '^androidx\.adversarial\.poc\.' "$ARTIFACTS_DIR/mapping.txt" || true)"
if [[ "$PACKAGE_HITS" -lt 1 ]]; then
    echo "ERROR: no class in mapping LHS starts with androidx.adversarial.poc — masquerade premise lost." >&2
    exit 3
fi
echo "  androidx.adversarial.poc.* class entries in mapping LHS: $PACKAGE_HITS"

echo "[4/4] METADATA.toml"
R8_VERSION="$($R8_CMD --version 2>&1 | head -1 || true)"
[[ -n "$R8_VERSION" ]] || R8_VERSION="unknown"
SOURCE_SHA="$(sha256sum "$SOURCE_DIR/Main.kt" | awk '{print $1}')"
RULES_SHA="$(sha256sum "$SOURCE_DIR/proguard-rules.pro" | awk '{print $1}')"
DEX_SHA="$(sha256sum "$ARTIFACTS_DIR/classes.dex" | awk '{print $1}')"
MAP_SHA="$(sha256sum "$ARTIFACTS_DIR/mapping.txt" | awk '{print $1}')"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat >"$ARTIFACTS_DIR/METADATA.toml" <<EOF
# Auto-generated by tests/fixtures/r8/family_prefix_masquerade/scripts/regen-family-prefix-masquerade-fixture.sh.
# Do not edit by hand.

r8_version = "$R8_VERSION"
generated_at = "$TIMESTAMP"
outline_annotation_count_global = $OUTLINE_COUNT
outline_annotation_count_adversarial = $ADVERSARIAL_OUTLINE_COUNT
androidx_adversarial_poc_class_entries = $PACKAGE_HITS

[source]
main_kt_sha256 = "$SOURCE_SHA"
proguard_rules_sha256 = "$RULES_SHA"

[artifacts]
classes_dex_sha256 = "$DEX_SHA"
mapping_txt_sha256 = "$MAP_SHA"
EOF

echo "Fixture regenerated."
echo "  classes.dex : $ARTIFACTS_DIR/classes.dex ($DEX_SHA)"
echo "  mapping.txt : $ARTIFACTS_DIR/mapping.txt ($MAP_SHA)"
echo "  METADATA    : $ARTIFACTS_DIR/METADATA.toml"
