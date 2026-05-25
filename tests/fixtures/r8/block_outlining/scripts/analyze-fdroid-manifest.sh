#!/usr/bin/env bash
# Analyze the F-Droid sweep TSV manifest.
#
# Replaces ad-hoc awk one-liners with a single repeatable invocation that
# emits all eight aggregate-stats sections in one pass. The live sweep
# harness (tests/r8_fdroid_apk_sweep.rs) writes the manifest at
#   $DROIDSAW_R8_FDROID_ROOT/swept-manifest-fdroid.tsv
# Column schema (header line starts with `package\t`; other `#` lines are
# free-form provenance comments):
#
#   package  apk_sha256  droidsaw_sha  timestamp_utc
#   dex_count  dex_sha256_list  class_count  decompile_fail_count
#   marker_count  distinct_helper_count  top10_helpers
#
# `top10_helpers` is a comma-separated list of `<descriptor>:<count>`
# pairs (max 10 per row, truncated at sweep-write time).
#
# Usage:
#   ./scripts/analyze-fdroid-manifest.sh [<path-to-manifest.tsv>]
#   ./scripts/analyze-fdroid-manifest.sh --demo
#
# With no path, defaults to
#   ${DROIDSAW_R8_FDROID_ROOT:-/home/shared/f-droid}/swept-manifest-fdroid.tsv
#
# --demo synthesises a 3-row fixture in /tmp/ and runs against that;
# this exercises parser + section structure without needing access to a
# live sweep host.
#
# Sections emitted (in order):
#   1. AGGREGATE                       — unique APKs, totals, rate/1000
#   2. F1/F2/F3 verdicts               — falsification-criterion gate
#   3. MARKER-RATE BIMODALITY HISTOGRAM
#   4. SIZE-BAND x RATE CROSS
#   5. TOP 20 FIRING DESCRIPTORS
#   6. NAMESPACE-LEVEL BREAKDOWN
#   7. CROSS-APK HELPER REUSE          — popular helpers (>=10 APKs)
#   8. DECOMPILE-FAIL OUTLIERS         — per-APK >1% fail rate
#
# Implementation notes:
#   * Pure bash + awk + sort. No python, no jq, no perl.
#   * The manifest is read once into a temp snapshot; the live sweep may
#     still be appending. Subsequent stages operate on the snapshot, so
#     analysis is deterministic across invocations on the same input.
#   * Dedup key is `apk_sha256`. The iteration model treats one pass at
#     any droidsaw commit as sufficient coverage; we ignore droidsaw_sha
#     for dedup. If a row appears more than once with the same apk_sha256
#     we keep the first occurrence (sweep determinism guarantees they
#     agree on counts, but keep-first is order-stable).
#   * F1/F2/F3 thresholds + KNOWN_FP_FAMILY mirror the harness consts in
#     tests/r8_fdroid_apk_sweep.rs. If you change them there, change them
#     here too (no shared source of truth — both are deliberately frozen
#     gauge anchors).

set -euo pipefail

# --- threshold + known-FP-family constants (mirror harness) -----------
F1_RATE_THRESHOLD_PER_1000="2.0"
F1_MIN_APKS_REQUIRED="20"
F2_OFF_FAMILY_FRACTION_THRESHOLD="0.30"
F3_DECOMPILE_FAIL_FRACTION_THRESHOLD="0.10"
# Comma-separated; passed to awk via -v. Matched by mapping-key prefix
# (entry `h1` matches key `h1` and `h1.foo`; entry `j$` matches `j$`,
# `j$.time`, `j$.com.android.tools.r8.DesugarVarHandle`, etc.). Mirror
# of tests/r8_fdroid_apk_sweep.rs::KNOWN_FP_FAMILY — keep in sync.
KNOWN_FP_FAMILY_CSV="h1,j\$.com.android.tools.r8,j\$,io.flutter,androidx"

# --- arg parsing ------------------------------------------------------
DEMO=0
INPUT=""
for arg in "$@"; do
    case "$arg" in
        --demo) DEMO=1 ;;
        -h|--help)
            sed -n '2,40p' "$0"
            exit 0
            ;;
        -*)
            echo "ERROR: unknown flag: $arg" >&2
            exit 2
            ;;
        *)
            if [[ -n "$INPUT" ]]; then
                echo "ERROR: extra positional arg: $arg" >&2
                exit 2
            fi
            INPUT="$arg"
            ;;
    esac
done

TMP="$(mktemp -d -t fdroid-analyze.XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

if [[ "$DEMO" -eq 1 ]]; then
    # Synthetic 3-row fixture that exercises every section: one bimodal-
    # high APK (lots of markers, off-family-heavy), one bimodal-zero
    # APK, one fail-outlier APK.
    INPUT="$TMP/demo-manifest.tsv"
    {
        echo "# DEMO synthetic manifest"
        printf "package\tapk_sha256\tdroidsaw_sha\ttimestamp_utc\tdex_count\tdex_sha256_list\tclass_count\tdecompile_fail_count\tmarker_count\tdistinct_helper_count\ttop10_helpers\n"
        printf "com.example.high\t%s\tdeadbeef\t2026-05-22T00:00:00Z\t1\tabc\t1000\t5\t50\t8\t%s\n" \
            "$(printf 'a%.0s' {1..64})" \
            "Lcom/example/Helper1;:20,Lcom/example/Helper2;:10,Lh1\$ext;:8,Lkotlin/Util;:5,Landroidx/core/util/Pair;:3,Lj\$/time/Instant;:2,Lcom/google/common/Foo;:1,Lokhttp3/Request;:1"
        printf "com.example.zero\t%s\tdeadbeef\t2026-05-22T00:00:00Z\t1\tabc\t500\t0\t0\t0\t\n" \
            "$(printf 'b%.0s' {1..64})"
        printf "com.example.fail\t%s\tdeadbeef\t2026-05-22T00:00:00Z\t1\tabc\t800\t20\t3\t2\t%s\n" \
            "$(printf 'c%.0s' {1..64})" \
            "Lcom/example/Bar;:2,Lcom/example/Baz;:1"
    } >"$INPUT"
elif [[ -z "$INPUT" ]]; then
    INPUT="${DROIDSAW_R8_FDROID_ROOT:-/home/shared/f-droid}/swept-manifest-fdroid.tsv"
fi

if [[ ! -f "$INPUT" ]]; then
    echo "ERROR: manifest not found: $INPUT" >&2
    echo "  Pass a path or set DROIDSAW_R8_FDROID_ROOT, or use --demo." >&2
    exit 2
fi

# Snapshot the manifest so a mid-sweep append doesn't shift counts
# between sections.
SNAPSHOT="$TMP/manifest.tsv"
cp "$INPUT" "$SNAPSHOT"

echo "# F-Droid sweep manifest analysis"
echo "# input: $INPUT"
echo "# snapshot bytes: $(wc -c <"$SNAPSHOT" | tr -d ' ')"
echo "# rows in snapshot (incl. comments + header): $(wc -l <"$SNAPSHOT" | tr -d ' ')"
echo

# ---------------------------------------------------------------------
# Single-pass awk: dedup on apk_sha256, accumulate per-band counters,
# emit one block per section. We pipe through awk twice — once for the
# data sections that need a sort step (TOP-20, popular-helpers,
# fail-outliers) and once for the AGGREGATE / verdict / histogram
# sections that are fully accumulator-driven.
# ---------------------------------------------------------------------

# Sections 1-4, 6: accumulator-only, no sort.
awk -F'\t' \
    -v f1_rate="$F1_RATE_THRESHOLD_PER_1000" \
    -v f1_min="$F1_MIN_APKS_REQUIRED" \
    -v f2_off="$F2_OFF_FAMILY_FRACTION_THRESHOLD" \
    -v f3_fail="$F3_DECOMPILE_FAIL_FRACTION_THRESHOLD" \
    -v kfp="$KNOWN_FP_FAMILY_CSV" '
function band_rate(rate,    b) {
    # rate is markers per class (raw, not per-1000). Bands defined per
    # 1000 in the spec; convert by *1000.
    b = rate * 1000.0
    if (b == 0)              return "0"
    if (b < 1.0)             return "0<r<1/k"
    if (b < 3.0)             return "1<=r<3/k"
    if (b < 10.0)            return "3<=r<10/k"
    return ">=10/k"
}
function band_size(c) {
    if (c < 500)             return "<500"
    if (c < 2000)            return "500-2k"
    if (c < 10000)           return "2k-10k"
    if (c < 30000)           return "10k-30k"
    return ">30k"
}
function descriptor_to_mapping_key(desc,    s) {
    # Lcom/example/Foo$Bar; -> com.example.Foo$Bar
    s = desc
    if (substr(s, 1, 1) == "L") s = substr(s, 2)
    if (substr(s, length(s), 1) == ";") s = substr(s, 1, length(s)-1)
    gsub("/", ".", s)
    return s
}
function in_known_fp(key, n, i, parts, prefix) {
    # Mirror tests/r8_fdroid_apk_sweep.rs::is_in_known_fp_family
    # EXACTLY: an entry `h1` matches the literal `h1` and the
    # `h1.<anything>` prefix; it does NOT match `h1$<anything>`
    # (harness rejects this) and it does NOT match `h10`.
    n = split(kfp, parts, ",")
    for (i = 1; i <= n; i++) {
        prefix = parts[i]
        if (key == prefix) return 1
        if (substr(key, 1, length(prefix)+1) == prefix ".") return 1
    }
    return 0
}
BEGIN {
    seen_count = 0
    total_classes = 0
    total_fails = 0
    total_markers = 0
    apks_with_markers = 0
}
# Skip blank lines.
NF == 0 { next }
# Skip comment lines.
substr($1, 1, 1) == "#" { next }
# Skip the column-name header line.
$1 == "package" { next }
{
    pkg    = $1
    sha    = $2
    classes = $7 + 0
    fails   = $8 + 0
    markers = $9 + 0
    top10   = $11

    if (sha == "" || sha in seen) next
    seen[sha] = 1
    seen_count++

    total_classes += classes
    total_fails   += fails
    total_markers += markers
    if (markers > 0) apks_with_markers++

    rate = (classes > 0) ? markers / classes : 0
    band = band_rate(rate)
    hist_count[band]++
    hist_total++

    sband = band_size(classes)
    size_apks[sband]++
    size_total++
    if (markers == 0) size_zero[sband]++
    size_rate_sum[sband] += rate

    if (classes > 0 && fails / classes > f3_fail + 0) {
        f3_trip_count++
        f3_trip_list[f3_trip_count] = sprintf("    %s  fails=%d  classes=%d  frac=%.4f",
            pkg, fails, classes, fails / classes)
    }

    # Walk top10_helpers for F2 family/off-family accounting only.
    # Namespace breakdown + TOP-20 + cross-APK reuse are computed in
    # later passes.
    if (top10 != "") {
        n = split(top10, pairs, ",")
        for (i = 1; i <= n; i++) {
            p = pairs[i]
            colon = index(p, ":")
            if (colon == 0) continue
            desc  = substr(p, 1, colon - 1)
            cnt   = substr(p, colon + 1) + 0
            key   = descriptor_to_mapping_key(desc)
            if (in_known_fp(key)) {
                family_markers += cnt
            } else {
                offfamily_markers += cnt
            }
        }
    }
}
END {
    # --- Section 1: AGGREGATE ----------------------------------------
    rate_per_1000 = (total_classes > 0) ? (total_markers * 1000.0 / total_classes) : 0
    printf "==== 1. AGGREGATE ====\n"
    printf "unique_apks=%d total_classes=%d decompile_fails=%d markers=%d apks_with_markers=%d rate_per_1000=%.2f\n",
        seen_count, total_classes, total_fails, total_markers, apks_with_markers, rate_per_1000
    print ""

    # --- Section 2: F1/F2/F3 verdicts --------------------------------
    # Operators mirror tests/r8_fdroid_apk_sweep.rs (harness uses
    # strict `>` against the threshold; this script must agree at
    # the boundary or verdicts drift between live-run and post-hoc).
    printf "==== 2. F1/F2/F3 VERDICTS ====\n"
    # F1: marker rate per 1000 classes, gated on min APK count.
    if (seen_count < f1_min + 0) {
        printf "F1 (rate > %s/1000 markers/class, min %s APKs): INSUFFICIENT (n=%d < %d)\n",
            f1_rate, f1_min, seen_count, f1_min + 0
    } else if (rate_per_1000 > f1_rate + 0) {
        printf "F1 (rate > %s/1000 markers/class, min %s APKs): TRIPPED (rate=%.2f)\n",
            f1_rate, f1_min, rate_per_1000
    } else {
        printf "F1 (rate > %s/1000 markers/class, min %s APKs): OK (rate=%.2f <= %s)\n",
            f1_rate, f1_min, rate_per_1000, f1_rate
    }
    # F2_TOP10_PROXY: off-family share of the TOP-10-PER-APK marker
    # subset only. NOT the harness F2 — the harness counts off-family
    # across ALL markers via the off_family_marker_count accumulator
    # which is NOT a column in the manifest. We can only approximate
    # over what is visible (top-10 per APK; ~78% of total markers at
    # the n=1203 checkpoint, less at deeper tails). Report as a proxy
    # so analysts do NOT read the verdict as harness-parity.
    total_subset = family_markers + offfamily_markers
    off_frac = (total_subset > 0) ? offfamily_markers / total_subset : 0
    if (total_subset == 0) {
        printf "F2_TOP10_PROXY (off-family frac > %s of top-10 subset): INSUFFICIENT (subset=0)\n", f2_off
    } else if (off_frac > f2_off + 0) {
        printf "F2_TOP10_PROXY (off-family frac > %s of top-10 subset): TRIPPED (off_frac=%.3f, off=%d, family=%d)\n",
            f2_off, off_frac, offfamily_markers, family_markers
    } else {
        printf "F2_TOP10_PROXY (off-family frac > %s of top-10 subset): OK (off_frac=%.3f, off=%d, family=%d)\n",
            f2_off, off_frac, offfamily_markers, family_markers
    }
    # F3: per-APK decompile-fail fraction; APKs that trip it were
    # buffered into f3_trip_list during the main pass.
    if (f3_trip_count + 0 == 0) {
        printf "F3 (per-APK decompile_fail/class_count > %s): OK (0 APKs trip)\n", f3_fail
    } else {
        printf "F3 (per-APK decompile_fail/class_count > %s): TRIPPED (%d APKs)\n",
            f3_fail, f3_trip_count
        for (i = 1; i <= f3_trip_count; i++) print f3_trip_list[i]
    }
    print ""

    # --- Section 3: MARKER-RATE BIMODALITY HISTOGRAM -----------------
    printf "==== 3. MARKER-RATE BIMODALITY HISTOGRAM ====\n"
    split("0|0<r<1/k|1<=r<3/k|3<=r<10/k|>=10/k", order, "|")
    for (i = 1; i <= 5; i++) {
        b = order[i]
        c = (b in hist_count) ? hist_count[b] : 0
        pct = (hist_total > 0) ? c * 100.0 / hist_total : 0
        printf "  %-12s  apks=%-6d  pct=%6.2f%%\n", b, c, pct
    }
    print ""

    # --- Section 4: SIZE-BAND x RATE CROSS ---------------------------
    printf "==== 4. SIZE-BAND x RATE CROSS ====\n"
    split("<500|500-2k|2k-10k|10k-30k|>30k", sorder, "|")
    for (i = 1; i <= 5; i++) {
        b = sorder[i]
        c = (b in size_apks) ? size_apks[b] : 0
        z = (b in size_zero) ? size_zero[b] : 0
        pz = (c > 0) ? z * 100.0 / c : 0
        mr = (c > 0) ? (size_rate_sum[b] * 1000.0 / c) : 0
        printf "  %-9s  apks=%-6d  pct_zero=%6.2f%%  mean_rate_per_1000=%.2f\n",
            b, c, pz, mr
    }
}
' "$SNAPSHOT"
echo

# ---------------------------------------------------------------------
# Section 5: TOP 20 FIRING DESCRIPTORS. Sum :N counts across all rows.
# ---------------------------------------------------------------------
echo "==== 5. TOP 20 FIRING DESCRIPTORS ===="
awk -F'\t' '
BEGIN { }
NF == 0 { next }
substr($1, 1, 1) == "#" { next }
$1 == "package" { next }
{
    sha = $2
    if (sha == "" || sha in seen) next
    seen[sha] = 1
    top10 = $11
    if (top10 == "") next
    n = split(top10, pairs, ",")
    for (i = 1; i <= n; i++) {
        p = pairs[i]
        colon = index(p, ":")
        if (colon == 0) continue
        desc = substr(p, 1, colon - 1)
        cnt  = substr(p, colon + 1) + 0
        totals[desc] += cnt
    }
}
END {
    for (d in totals) printf "%d\t%s\n", totals[d], d
}
' "$SNAPSHOT" | sort -k1,1nr | awk -F'\t' '
BEGIN { rows = 0 }
NR <= 20 { printf "  %-8d  %s\n", $1, $2; rows++ }
END { if (rows == 0) print "  (no top-10 helper data in manifest)" }
'
echo

# ---------------------------------------------------------------------
# Section 6: NAMESPACE-LEVEL BREAKDOWN. Aggregate per-namespace marker
# counts + distinct-descriptor counts over the top-10 subset.
# ---------------------------------------------------------------------
echo "==== 6. NAMESPACE-LEVEL BREAKDOWN ===="
echo "  (computed across the top-10-helpers subset only; per-APK"
echo "   truncation at 10 bounds the marker accounting.)"
awk -F'\t' '
function descriptor_to_mapping_key(desc,    s) {
    s = desc
    if (substr(s, 1, 1) == "L") s = substr(s, 2)
    if (substr(s, length(s), 1) == ";") s = substr(s, 1, length(s)-1)
    gsub("/", ".", s)
    return s
}
function namespace_of(key,    rest) {
    if (substr(key, 1, 9) == "androidx.") {
        rest = substr(key, 10)
        sub(/\..*$/, "", rest)
        return "androidx." rest
    }
    if (substr(key, 1, 3) == "j$.") {
        rest = substr(key, 4)
        sub(/\..*$/, "", rest)
        return "j$." rest
    }
    if (substr(key, 1, 7) == "kotlin.")        return "kotlin"
    if (substr(key, 1, 11) == "com.google.")   return "com.google"
    if (substr(key, 1, 3) == "io.")            return "io"
    if (substr(key, 1, 12) == "com.squareup.") return "com.squareup"
    if (substr(key, 1, 8) == "okhttp3.")       return "okhttp3"
    if (substr(key, 1, 9) == "retrofit.")      return "retrofit"
    return "other"
}
NF == 0 { next }
substr($1, 1, 1) == "#" { next }
$1 == "package" { next }
{
    sha = $2
    if (sha == "" || sha in seen) next
    seen[sha] = 1
    top10 = $11
    if (top10 == "") next
    n = split(top10, pairs, ",")
    for (i = 1; i <= n; i++) {
        p = pairs[i]
        colon = index(p, ":")
        if (colon == 0) continue
        desc = substr(p, 1, colon - 1)
        cnt  = substr(p, colon + 1) + 0
        key  = descriptor_to_mapping_key(desc)
        ns   = namespace_of(key)
        ns_markers[ns] += cnt
        nsd_key = ns "\t" desc
        if (!(nsd_key in nsd_seen)) {
            nsd_seen[nsd_key] = 1
            ns_distinct[ns]++
        }
        ns_total += cnt
    }
}
END {
    for (ns in ns_markers) {
        pct = (ns_total > 0) ? ns_markers[ns] * 100.0 / ns_total : 0
        printf "%d\t%s\t%d\t%.2f\n", ns_markers[ns], ns, ns_distinct[ns], pct
    }
}
' "$SNAPSHOT" | sort -k1,1nr | awk -F'\t' '
BEGIN { rows = 0 }
{
    printf "  %-22s  markers=%-8d  distinct=%-6d  pct_of_subset=%6.2f%%\n", $2, $1, $3, $4
    rows++
}
END {
    if (rows == 0) print "  (no top-10 helper data in manifest)"
}
'
echo

# ---------------------------------------------------------------------
# Section 7: CROSS-APK HELPER REUSE. Distinct-APK count per descriptor;
# emit descriptors that appear in >=10 APKs with total hits + APK count.
# ---------------------------------------------------------------------
echo "==== 7. CROSS-APK HELPER REUSE (descriptors in >=10 APKs) ===="
awk -F'\t' '
NF == 0 { next }
substr($1, 1, 1) == "#" { next }
$1 == "package" { next }
{
    sha = $2
    if (sha == "" || sha in seen) next
    seen[sha] = 1
    top10 = $11
    if (top10 == "") next
    n = split(top10, pairs, ",")
    for (i = 1; i <= n; i++) {
        p = pairs[i]
        colon = index(p, ":")
        if (colon == 0) continue
        desc = substr(p, 1, colon - 1)
        cnt  = substr(p, colon + 1) + 0
        totals[desc] += cnt
        key = desc "\t" sha
        if (!(key in seen_pair)) {
            seen_pair[key] = 1
            apk_count[desc]++
        }
    }
}
END {
    for (d in apk_count) {
        if (apk_count[d] >= 10) {
            printf "%d\t%d\t%s\n", apk_count[d], totals[d], d
        }
    }
}
' "$SNAPSHOT" | sort -k1,1nr -k2,2nr | awk -F'\t' '
BEGIN { rows = 0 }
{
    printf "  apks=%-6d  hits=%-8d  %s\n", $1, $2, $3
    rows++
}
END {
    if (rows == 0) print "  (none -- no descriptor appears in 10+ APKs)"
}
'
echo

# ---------------------------------------------------------------------
# Section 8: DECOMPILE-FAIL OUTLIERS. fails / classes > 0.01.
# ---------------------------------------------------------------------
echo "==== 8. DECOMPILE-FAIL OUTLIERS (fails/classes > 1%) ===="
awk -F'\t' '
NF == 0 { next }
substr($1, 1, 1) == "#" { next }
$1 == "package" { next }
{
    pkg     = $1
    sha     = $2
    classes = $7 + 0
    fails   = $8 + 0
    if (sha == "" || sha in seen) next
    seen[sha] = 1
    if (classes <= 0) next
    frac = fails / classes
    if (frac > 0.01) {
        printf "%.6f\t%s\t%d\t%d\n", frac, pkg, fails, classes
    }
}
' "$SNAPSHOT" | sort -k1,1nr | awk -F'\t' '
BEGIN { rows = 0 }
{
    printf "  frac=%.4f  pkg=%-40s  fails=%-6d  classes=%-8d\n", $1, $2, $3, $4
    rows++
}
END {
    if (rows == 0) print "  (none above 1%)"
}
'
echo

# ---------------------------------------------------------------------
# Section 9: PRECISE NAMESPACE F2 (when namespace_rollup column present).
#
# Reads column 12 (namespace_rollup) when it exists. Each row contains
# every bucket explicitly so totals are positional and recomputation
# does NOT depend on the truncated top-10. F2_PRECISE is the off-family
# fraction over the FULL marker stream (not the top-10 proxy).
#
# Rows lacking namespace_rollup are silently skipped (older sweep
# commits predate the column); the section prints a coverage note so
# you can see how many rows contributed.
# ---------------------------------------------------------------------
echo "==== 9. PRECISE NAMESPACE F2 (namespace_rollup column) ===="
awk -F'\t' '
NF == 0 { next }
substr($1, 1, 1) == "#" { next }
$1 == "package" { next }
NF < 12 || $12 == "" { rows_without_rollup++; next }
{
    sha = $2
    if (sha == "" || sha in seen) next
    seen[sha] = 1
    rollup = $12
    n = split(rollup, pairs, ",")
    for (i = 1; i <= n; i++) {
        eq = index(pairs[i], "=")
        if (eq == 0) continue
        bucket = substr(pairs[i], 1, eq-1)
        ct = substr(pairs[i], eq+1) + 0
        total[bucket] += ct
        if (bucket == "h1" || bucket == "j$.com.android.tools.r8" || bucket == "j$" || bucket == "io.flutter" || bucket == "androidx") {
            family += ct
        } else {
            offfamily += ct
        }
    }
    rows_with_rollup++
}
END {
    if (rows_with_rollup == 0) {
        print "  (no rows carry namespace_rollup column; pre-column sweeps only)"
        if (rows_without_rollup > 0) {
            printf "  rows_without_rollup=%d (pre-12-column schema)\n", rows_without_rollup
        }
        exit 0
    }
    grand = family + offfamily
    printf "  rows_with_rollup=%d  rows_without_rollup=%d\n", rows_with_rollup, rows_without_rollup
    printf "  total markers (across rollup rows): %d\n", grand
    printf "  family (h1+j$.com.android.tools.r8+j$+io.flutter+androidx): %d (%.1f%%)\n", family, (grand>0?100.0*family/grand:0)
    printf "  off-family:                          %d (%.3f frac)\n", offfamily, (grand>0?1.0*offfamily/grand:0)
    if (grand > 0) {
        frac = 1.0*offfamily/grand
        verdict = (frac > 0.30) ? "TRIPPED" : "OK"
        printf "  F2_PRECISE (frac > 0.30): %s\n", verdict
    }
    print "  --- per-bucket totals ---"
    # Stable bucket order to match harness emit order.
    bkts = "h1 j$.com.android.tools.r8 j$ io.flutter androidx kotlin kotlinx com.google com.android dagger other"
    nb = split(bkts, ba, " ")
    for (i = 1; i <= nb; i++) {
        b = ba[i]
        printf "  %-12s = %d\n", b, total[b]+0
    }
}
' "$SNAPSHOT"
echo

# ---------------------------------------------------------------------
# Section 10: PER-BUCKET ATTESTATION RATE (masquerade-window probe).
#
# Reads column 13 (namespace_rollup_attested) and column 12
# (namespace_rollup). For each bucket, prints attested/total markers
# and the ratio. Low ratios = wide masquerade window: an attacker
# naming a class with that bucket's prefix and structuring it as an
# R8 outline (I4–I13 invariants) would slip past the FP-suppression
# filter in mapping-less analysis. See KNOWN_FP_FAMILY threat-model
# docstring + tests/r8_fdroid_apk_sweep.rs.
#
# In mapping-less sweeps most markers lack structural attestation
# (R8 typically strips $$ExternalSynthetic infix under minification);
# the absolute attestation rate is therefore expected to be low. The
# RATIO is the signal — buckets with very different attestation
# rates indicate different defence postures per ecosystem.
# ---------------------------------------------------------------------
echo "==== 10. PER-BUCKET ATTESTATION RATE (masquerade window) ===="
awk -F'\t' '
NF == 0 { next }
substr($1, 1, 1) == "#" { next }
$1 == "package" { next }
NF < 13 || $12 == "" || $13 == "" { rows_skipped++; next }
{
    sha = $2
    if (sha == "" || sha in seen) next
    seen[sha] = 1
    rollup = $12
    n = split(rollup, pairs, ",")
    for (i = 1; i <= n; i++) {
        eq = index(pairs[i], "=")
        if (eq == 0) continue
        bucket = substr(pairs[i], 1, eq-1)
        ct = substr(pairs[i], eq+1) + 0
        total[bucket] += ct
    }
    rollup_a = $13
    n = split(rollup_a, pairs, ",")
    for (i = 1; i <= n; i++) {
        eq = index(pairs[i], "=")
        if (eq == 0) continue
        bucket = substr(pairs[i], 1, eq-1)
        ct = substr(pairs[i], eq+1) + 0
        attested[bucket] += ct
    }
    rows_with_both++
}
END {
    if (rows_with_both == 0) {
        print "  (no rows carry namespace_rollup_attested column; pre-13-column schema only)"
        if (rows_skipped > 0) {
            printf "  rows_without_attested=%d (pre-13-column schema)\n", rows_skipped
        }
        exit 0
    }
    printf "  rows_with_attested=%d  rows_without_attested=%d\n", rows_with_both, rows_skipped
    printf "  %-12s   %12s   %12s   %8s\n", "bucket", "total", "attested", "ratio"
    bkts = "h1 j$.com.android.tools.r8 j$ io.flutter androidx kotlin kotlinx com.google com.android dagger other"
    nb = split(bkts, ba, " ")
    for (i = 1; i <= nb; i++) {
        b = ba[i]
        t = total[b] + 0
        a = attested[b] + 0
        ratio_str = (t > 0) ? sprintf("%.4f", 1.0 * a / t) : "n/a"
        printf "  %-12s   %12d   %12d   %8s\n", b, t, a, ratio_str
    }
}
' "$SNAPSHOT"
echo

echo "# analysis complete."
