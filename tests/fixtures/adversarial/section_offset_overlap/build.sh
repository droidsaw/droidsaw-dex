#!/usr/bin/env bash
# Build the section-offset-overlap PoC fixtures from src.java.
# Mirrors tests/fixtures/build_fixture.sh (ANDROID_HOME discovery, javac -g
# --release 8, d8 --no-desugaring).
#
# Produces, in this directory:
#   base.dex                        — benign SectionOverlap class
#   method_ids_aliases_field_ids.dex — base.dex with method_ids_off := field_ids_off
#                                      and the Adler-32 re-sealed (the adversarial
#                                      input; see README.md for the full recipe)
#
# Usage: ./build.sh   (or: ANDROID_HOME=~/android-sdk ./build.sh)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC="$SCRIPT_DIR/src.java"
BASE="$SCRIPT_DIR/base.dex"
OVERLAP="$SCRIPT_DIR/method_ids_aliases_field_ids.dex"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Find d8 (same discovery as build_fixture.sh).
if [ -n "${ANDROID_HOME:-}" ]; then
    D8="$(find "$ANDROID_HOME/build-tools" -name d8 -type f 2>/dev/null | sort -V | tail -1)"
else
    D8="$(command -v d8 2>/dev/null || true)"
fi
if [ -z "${D8:-}" ]; then
    echo "error: d8 not found. Set ANDROID_HOME or add d8 to PATH." >&2
    exit 1
fi

# `src.java` declares a public class; javac requires the file basename to match,
# so compile through a renamed copy (the on-disk source stays `src.java` per the
# tests/fixtures/java/<Case>/src.java convention).
cp "$SRC" "$TMP/SectionOverlap.java"
javac -g --release 8 -d "$TMP" "$TMP/SectionOverlap.java"
mkdir -p "$TMP/dex"
"$D8" --no-desugaring --output "$TMP/dex" "$TMP/SectionOverlap.class"
cp "$TMP/dex/classes.dex" "$BASE"
echo "wrote $BASE ($(wc -c < "$BASE") bytes)"

# Adversarial mutation: alias method_ids onto field_ids (same 8-byte item
# shape), then re-seal the Adler-32 so the corruption — not a checksum
# mismatch — is what a parser reacts to. Header field offsets: field_ids_off
# @0x54, method_ids_off @0x5c, file_size @0x20, checksum @0x08. The SHA-1
# signature is intentionally NOT re-sealed (DexFile::parse ignores it).
python3 - "$BASE" "$OVERLAP" <<'PY'
import sys, struct, zlib
src, dst = sys.argv[1], sys.argv[2]
d = bytearray(open(src, 'rb').read())
rd = lambda o: struct.unpack_from('<I', d, o)[0]
struct.pack_into('<I', d, 0x5c, rd(0x54))                 # method_ids_off := field_ids_off
struct.pack_into('<I', d, 0x08, zlib.adler32(bytes(d[12:rd(0x20)])))   # re-seal Adler-32
open(dst, 'wb').write(bytes(d))
PY
echo "wrote $OVERLAP ($(wc -c < "$OVERLAP") bytes)"
