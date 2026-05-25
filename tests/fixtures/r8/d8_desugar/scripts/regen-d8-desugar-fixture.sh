#!/usr/bin/env bash
# Regenerate the D8 desugar fixture artifacts.
#
# Inputs: source/Main.kt + source/proguard-rules.pro
# Outputs:
#   artifacts/classes.dex
#   artifacts/mapping.txt
#   artifacts/METADATA.toml
#
# Pipeline:
#   1. kotlinc Main.kt -> classes.jar (JVM class files)
#   2. d8 classes.jar --min-api 24 --desugared-lib <json> -> step1.dex
#      (this is where the j$/* backports get emitted — D8's desugaring
#      step is what creates the Lj$/time/, Lj$/util/stream/ classes)
#   3. r8 --release --pg-conf rules --pg-map-output mapping.txt
#      --desugared-lib <json> step1.dex -> classes.dex
#      (R8 sees the j$/* backports as ordinary input classes; the
#      structural claim is that R8 does NOT emit additional methods
#      into the j$/* namespace via outlining)
#   4. Record METADATA (R8/D8 versions, source sha256, counts).
#
# Min-api choice: 24. At min-api 24, java.time (API 26+) is desugared;
# java.util.stream gets the partial backport set the desugared-library
# config covers; java.util.Optional is native (API 24+) and not
# desugared. This is enough to populate the j$/* namespace with the
# time + stream backports — the dominant families.
#
# Requirements (must be on PATH or pinned via env vars):
#   kotlinc       — Kotlin compiler (1.9+)
#   d8            — Android D8 CLI (or pin via $D8_JAR)
#   r8            — Android R8 CLI (or pin via $R8_JAR)
#   sha256sum     — coreutils
#   $DESUGARED_LIB_JSON  — desugar_jdk_libs_configuration.json (required)
#   $DESUGARED_LIB_JAR   — desugar_jdk_libs.jar prebuilt (required for L8 link)
#
# The script is idempotent. Run it after editing source/Main.kt or
# bumping R8 / D8 / the desugared-library config.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCE_DIR="$FIXTURE_ROOT/source"
ARTIFACTS_DIR="$FIXTURE_ROOT/artifacts"
TMP="$(mktemp -d -t d8desugarfixture.XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "ERROR: $1 not on PATH." >&2
        exit 2
    fi
}

require_env() {
    if [[ -z "${!1:-}" ]]; then
        echo "ERROR: env var \$$1 is required. See README.md." >&2
        exit 2
    fi
    if [[ ! -f "${!1}" ]]; then
        echo "ERROR: \$$1=${!1} but the file does not exist." >&2
        exit 2
    fi
}

require_tool kotlinc
require_tool sha256sum
require_env DESUGARED_LIB_JSON
require_env DESUGARED_LIB_JAR

# D8 dispatch.
D8_CMD=""
if [[ -n "${D8_JAR:-}" ]]; then
    if [[ ! -f "$D8_JAR" ]]; then
        echo "ERROR: \$D8_JAR=$D8_JAR but the file does not exist." >&2
        exit 2
    fi
    D8_CMD="java -cp $D8_JAR com.android.tools.r8.D8"
elif command -v d8 >/dev/null 2>&1; then
    D8_CMD="d8"
else
    echo "ERROR: neither \$D8_JAR nor the 'd8' wrapper is available." >&2
    echo "  Install Android SDK build-tools OR set D8_JAR=/path/to/d8.jar." >&2
    echo "  D8 and R8 ship in the same jar; the burrow's r8-9.0.32.jar" >&2
    echo "  works for both — set D8_JAR=R8_JAR." >&2
    exit 2
fi

# R8 dispatch.
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

# Library jar (android.jar or JDK home).
R8_LIB=""
if [[ -n "${ANDROID_JAR:-}" ]]; then
    R8_LIB="$ANDROID_JAR"
elif [[ -n "${ANDROID_HOME:-}" ]]; then
    R8_LIB="$(ls -d "$ANDROID_HOME"/platforms/android-*/android.jar 2>/dev/null | sort -V | tail -1)"
fi
if [[ -z "$R8_LIB" ]]; then
    # awk exits on first match so head -1 isn't needed; the prior
    # `| head -1` form would SIGPIPE the upstream awk under
    # `set -o pipefail` (same shape as the prior
    # analyze-fdroid-manifest.sh SIGPIPE bug).
    R8_LIB="$(java -XshowSettings:properties -version 2>&1 | awk -F' = ' '/java.home/{print $2; exit}')"
fi
if [[ -z "$R8_LIB" || ! -e "$R8_LIB" ]]; then
    echo "ERROR: could not resolve --lib target." >&2
    exit 2
fi
echo "Lib: $R8_LIB"
echo "Desugared lib JSON: $DESUGARED_LIB_JSON"
echo "Desugared lib JAR:  $DESUGARED_LIB_JAR"

mkdir -p "$ARTIFACTS_DIR"

echo "[1/5] kotlinc -> $TMP/classes.jar"
kotlinc "$SOURCE_DIR/Main.kt" -d "$TMP/classes.jar"

# Locate the Kotlin runtime jars.
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

echo "[2/5] d8 --min-api 24 --desugared-lib -> $TMP/d8out"
# D8's desugaring step is what creates the Lj$/time/, Lj$/util/stream/
# backport classes in the output DEX. The --desugared-lib flag wires
# in the L8 configuration that tells D8 which java.* APIs to backport
# and how to name the j$/* equivalents.
mkdir -p "$TMP/d8out"
$D8_CMD \
    --min-api 24 \
    --lib "$R8_LIB" \
    --classpath "$KOTLIN_STDLIB" \
    --classpath "$KOTLIN_ANNOTATIONS" \
    --desugared-lib "$DESUGARED_LIB_JSON" \
    --output "$TMP/d8out" \
    "$TMP/classes.jar" \
    "$KOTLIN_STDLIB" \
    "$KOTLIN_ANNOTATIONS"

if [[ ! -f "$TMP/d8out/classes.dex" ]]; then
    echo "ERROR: d8 did not emit classes.dex (look in $TMP/d8out)." >&2
    exit 3
fi

echo "[3/5] r8 --release --desugared-lib -> $TMP/r8out"
# R8 sees the j$/* backports as ordinary input classes. The
# structural claim being anchored: R8 does NOT emit additional
# methods into the j$/* namespace via outlining. The
# --desugared-lib flag here passes the same L8 configuration so R8
# treats the j$/* namespace consistently.
mkdir -p "$TMP/r8out"
$R8_CMD --release \
    --min-api 24 \
    --lib "$R8_LIB" \
    --classpath "$KOTLIN_STDLIB" \
    --classpath "$KOTLIN_ANNOTATIONS" \
    --desugared-lib "$DESUGARED_LIB_JSON" \
    --pg-conf "$SOURCE_DIR/proguard-rules.pro" \
    --pg-map-output "$ARTIFACTS_DIR/mapping.txt" \
    --output "$TMP/r8out" \
    "$TMP/d8out/classes.dex"

if [[ ! -f "$TMP/r8out/classes.dex" ]]; then
    echo "ERROR: r8 did not emit classes.dex (look in $TMP/r8out)." >&2
    exit 3
fi
cp "$TMP/r8out/classes.dex" "$ARTIFACTS_DIR/classes.dex"

echo "[4/5] sanity-check mapping shape"
# Count j$ class records (LHS prefix `j$.` since mapping.txt uses
# source-form names with `.` separators).
JDOLLAR_CLASS_COUNT="$(grep -cE '^j\$\.' "$ARTIFACTS_DIR/mapping.txt" || true)"
OUTLINE_COUNT="$(grep -c 'com.android.tools.r8.outline"' "$ARTIFACTS_DIR/mapping.txt" || true)"

if [[ "$JDOLLAR_CLASS_COUNT" -lt 1 ]]; then
    echo "ERROR: mapping.txt has 0 j\$.* class records." >&2
    echo "       D8 desugaring did not fire — check the --min-api setting" >&2
    echo "       and the DESUGARED_LIB_JSON path. The fixture's purpose is to" >&2
    echo "       anchor the j\$/* claim; without j\$ classes there is no anchor." >&2
    exit 3
fi

if [[ "$OUTLINE_COUNT" -lt 1 ]]; then
    echo "ERROR: mapping.txt has 0 com.android.tools.r8.outline annotations." >&2
    echo "       Positive-control assertion 5 in the ratchet will fail with" >&2
    echo "       these artifacts. Refusing to commit broken anchors." >&2
    exit 3
fi

echo "[5/5] METADATA.toml"
R8_VERSION="$($R8_CMD --version 2>&1 | head -1 || true)"
D8_VERSION="$($D8_CMD --version 2>&1 | head -1 || true)"
[[ -n "$R8_VERSION" ]] || R8_VERSION="unknown"
[[ -n "$D8_VERSION" ]] || D8_VERSION="unknown"
SOURCE_SHA="$(sha256sum "$SOURCE_DIR/Main.kt" | awk '{print $1}')"
RULES_SHA="$(sha256sum "$SOURCE_DIR/proguard-rules.pro" | awk '{print $1}')"
DESUGAR_JSON_SHA="$(sha256sum "$DESUGARED_LIB_JSON" | awk '{print $1}')"
DESUGAR_JAR_SHA="$(sha256sum "$DESUGARED_LIB_JAR" | awk '{print $1}')"
DEX_SHA="$(sha256sum "$ARTIFACTS_DIR/classes.dex" | awk '{print $1}')"
MAP_SHA="$(sha256sum "$ARTIFACTS_DIR/mapping.txt" | awk '{print $1}')"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat >"$ARTIFACTS_DIR/METADATA.toml" <<EOF
# Auto-generated by tests/fixtures/r8/d8_desugar/scripts/regen-d8-desugar-fixture.sh.
# Do not edit by hand; re-run the script after touching source/.

r8_version = "$R8_VERSION"
d8_version = "$D8_VERSION"
min_api = 24
generated_at = "$TIMESTAMP"
jdollar_class_count = $JDOLLAR_CLASS_COUNT
outline_annotation_count = $OUTLINE_COUNT

[source]
main_kt_sha256 = "$SOURCE_SHA"
proguard_rules_sha256 = "$RULES_SHA"
desugared_lib_json_sha256 = "$DESUGAR_JSON_SHA"
desugared_lib_jar_sha256 = "$DESUGAR_JAR_SHA"

[artifacts]
classes_dex_sha256 = "$DEX_SHA"
mapping_txt_sha256 = "$MAP_SHA"
EOF

echo "Fixture regenerated."
echo "  classes.dex : $ARTIFACTS_DIR/classes.dex ($DEX_SHA)"
echo "  mapping.txt : $ARTIFACTS_DIR/mapping.txt ($MAP_SHA)"
echo "    j\$.* class records: $JDOLLAR_CLASS_COUNT"
echo "    outline annotations: $OUTLINE_COUNT"
echo "  METADATA    : $ARTIFACTS_DIR/METADATA.toml"
