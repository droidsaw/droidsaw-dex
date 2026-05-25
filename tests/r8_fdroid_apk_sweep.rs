//! Env-gated FP-only sweep across the F-Droid APK-blob corpus.
//!
//! Sibling to [`r8_product_corpus_sweep`]. Same shape (per-APK
//! marker accounting + TSV manifest), but the input layout
//! differs: F-Droid is a content-addressed APK store, not a tree
//! of extracted DEX directories. The harness reads the APK index,
//! opens each blob as a zip, and pulls `classes*.dex` entries into
//! memory before running the recogniser.
//!
//! Reads `$DROIDSAW_R8_FDROID_ROOT` (the F-Droid mirror root,
//! containing `index/manifest-latest.tsv` + `blobs/<aa>/<sha256>.apk`).
//! Appends one row per APK to `$ROOT/swept-manifest-fdroid.tsv`.
//!
//! This is a **mapping-less** sweep — F-Droid does not publish R8
//! mappings publicly. Per-individual-marker TP/FP labels are
//! unavailable; the sweep produces aggregate calibration data
//! feeding the FP-ceiling arm of the validation gauge at corpus
//! scale.
//!
//! # Sample mode
//!
//! For sanity-test before full-corpus runs, set
//! `DROIDSAW_R8_FDROID_SAMPLE_N=<count>` to cap the number of APKs
//! processed. Unset = full corpus. Picks the FIRST `count` rows
//! from the manifest after stable sort by sha256 (deterministic
//! sub-sample for reproducibility).
//!
//! # Resumability
//!
//! The harness reads any existing `swept-manifest-fdroid.tsv` at
//! start and builds a set of `(droidsaw_sha, apk_sha256)` pairs
//! already processed. APKs in that set are skipped — a re-run on
//! the same commit picks up where the previous run left off.
//! Cross-commit re-runs do NOT skip (different droidsaw_sha means
//! different recogniser version → fresh data).
//!
//! # Adversarial-input discipline
//!
//! - Symlink reject on every blob open.
//! - 64 MiB per-DEX size cap when extracting from the APK zip.
//! - 64 MiB per-APK zip read cap (zip crate's
//!   `with_max_compressed_size` would do this more cleanly, but
//!   the raw blob is already on disk so we cap the file size
//!   instead).
//! - 16 MiB worker thread stack.
//! - F1/F2/F3 falsification criteria are evaluated at end of sweep
//!   + logged; they do not panic.
//! - I/O failures writing the manifest or reading rows that should
//!   exist (manifest claims an APK but the blob is missing) DO
//!   panic — they invalidate the data.

use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod common;

use common::r8_canonical_marker::{
    descriptor_to_mapping_key, parse_block_outlined_marker,
};
use common::r8_mapping_outline::{classify_synthetic_kind, SyntheticKind};

const MAX_APK_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DEX_BYTES: usize = 64 * 1024 * 1024;
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;
const MANIFEST_FILENAME: &str = "swept-manifest-fdroid.tsv";
const INDEX_RELATIVE_PATH: &str = "index/manifest-latest.tsv";

/// Known-FP descriptor family. Adding entries requires structural
/// justification — see [[dex-r8-product-corpus-sweep]] Q3.
///
/// ## Threat model — this is a FP-reduction hint, NOT a security boundary.
///
/// In mapping-less sweeps the recogniser fires on outline-shape methods
/// (I4–I13 structural invariants) regardless of class name; the
/// `KNOWN_FP_FAMILY` prefix check suppresses the marker as "looks like
/// legitimate library code." An attacker controlling DEX class names
/// can exploit this by naming a malicious helper with a family prefix
/// (e.g. `Landroidx/evil/Backdoor;`, `Lj$/com/evil/Backdoor;`,
/// `Lio/flutter/evil/Backdoor;`) and structuring its body to satisfy
/// the I4–I13 invariants (≥20 distinct callers, ACC_PUBLIC|ACC_STATIC,
/// straight-line body, narrow type signature). The marker is then
/// suppressed in mapping-less analysis. DEX has no namespace
/// enforcement: an attacker can compile `Lj$/...` classes directly via
/// smali or raw bytecode toolchains. R8/D8 owns these namespaces by
/// convention, not by runtime restriction.
///
/// **Ground-truth gauge: mapping-paired analysis.** When `mapping.txt`
/// is available, classify markers via the `$$ExternalSynthetic<kind>`
/// infix or the `com.android.tools.r8.outline` annotation on the LHS
/// (original) class name. Both are R8-emitted; an attacker without
/// control of the R8 toolchain cannot forge them. Use
/// [`classify_synthetic_kind`] over the `mapping.txt` LHS as the
/// classifier in mapping-paired contexts.
///
/// **Mapping-less mitigation: attestation rate.** The
/// `namespace_rollup_attested` column counts only markers where the
/// structural classifier returns non-Unknown (i.e., the obfuscated
/// name still carried `$EnumUnboxingLocalUtility` suffix or
/// `$$ExternalSynthetic` infix despite minification). The ratio
/// `attested[bucket] / total[bucket]` is the attestation rate per
/// family — low rates signal a wide masquerade window. R8 frequently
/// strips the `$$ExternalSynthetic` infix in obfuscated form, so even
/// legitimate library helpers often have low attestation; absent
/// mapping.txt the rate is a HINT to the analyst, not a verdict.
///
/// **Empirical attestation rates** (n=643 targeted-rerun cohort,
/// droidsaw_sha `313e84450`, F-Droid post-family-broaden + post-attested
/// column landing):
///
/// - `h1`:         0 / 79    = **0.00%**
/// - `j$`:         0 / 1,159 = **0.00%**
/// - `io.flutter`: 0 / 1,509 = **0.00%**
/// - `androidx`:   162 / 8,410 = **1.93%** (via EnumUnboxing-shape detection)
///
/// `h1`, `j$`, and `io.flutter` family prefixes carry zero structural
/// fallback in mapping-less analysis — a class arbitrarily named under
/// those prefixes is suppressed from the family-vs-off-family tallies
/// without an in-bucket attestation signal. `androidx` retains a 1.93%
/// structural defence via the inner-class `$EnumUnboxingLocalUtility`
/// naming pattern that R8 preserves under minification. The 162 attested
/// markers across that bucket concentrate in enum-shaped helpers
/// (`androidx.work.NetworkType$EnumUnboxingLocalUtility` and similar);
/// non-enum masquerade in `androidx` retains 0% structural attestation.
///
/// **Defense in depth.** Family suppression does not hide the
/// attacker's code from the decompiler's output — it only changes
/// how the marker is bucketed in the sweep summary. Cross-reference
/// analysis, call-graph extraction, taint tracking, entropy /
/// packer detectors, and string-table inspection all continue to
/// operate on the malicious class. The family is one lens among
/// many; do not rely on it as a sole filter when adversaries control
/// the input.
///
/// **`ACC_SYNTHETIC` gate raises the masquerade bar.** The recogniser
/// (`r8_inversion.rs:796`) rejects any helper class without
/// `ACC_SYNTHETIC` (0x1000) set. kotlinc-emitted developer classes
/// don't carry that flag, so naively naming a Kotlin class in a
/// family-prefix namespace does NOT trigger the recogniser at all —
/// the family suppression is never asked. A complete masquerade
/// requires the attacker to also forge `ACC_SYNTHETIC`, achievable
/// via smali assembly or raw DEX emit but harder than the bare
/// rename. This is empirically anchored by
/// `tests/fixtures/r8/family_prefix_masquerade/` — a negative-control
/// fixture where the naive masquerade is built and the recogniser
/// is asserted to NOT fire. A regression that drops or weakens the
/// gate causes that ratchet to fail.
///
/// ## Members:
/// - `h1` — horizontal-merge bridge (synthesized but NOT outline-
///   annotated; observed FP on a flashcard app with published mappings).
/// - `j$` — D8 desugared-library namespace, entire. Per R8 source
///   `LibraryDesugaredChecker.java:81`: library-desugared is
///   defined as exactly `startsWith("Lj$/")`. Per
///   `SyntheticNaming.java:104-139` outline-flavoured SyntheticKinds
///   (OUTLINE, COVARIANT_OUTLINE, OBJECT_CLONE_OUTLINE, API_MODEL_*,
///   NON_STARTUP_IN_STARTUP_OUTLINE, BOTTOM_UP_OUTLINE) build
///   synthetic names via `createExternalType(host, "$$ExternalSynthetic" + kind, id)`
///   on the host class — no `j$` injection path exists. The
///   empirical case `Lj$/com/android/tools/r8/<class>;` (top
///   descriptor in the F-Droid sweep at 490 hits across 95 APKs)
///   is R8's own `DesugarVarHandle` / `DesugarMethodHandlesLookup`
///   helpers re-homed under `j$/` via the L8 library-desugaring
///   naming lens — D8 emit of R8's runtime helpers, NOT R8 outline
///   into `j$/`. Anchored empirically by the
///   `tests/fixtures/r8/d8_desugar/` fixture.
/// - `io.flutter` — Flutter Android engine helper namespace.
///   Per `flutter/packages/flutter_tools/gradle/flutter_proguard_rules.pro`
///   the Flutter keep rules are `-dontwarn` + a
///   `FlutterPlugin`-implementor rule with
///   `allowshrinking,allowobfuscation` (NO surface-preserving
///   keep). `flutter_embedding.jar` is a Maven `api` dep that R8
///   sees as program input and minifies per-app. The 22+ APK
///   reuse of `Lio/flutter/plugin/platform/i;` in the F-Droid
///   sweep is `same input jar -> same R8 rename across apps`,
///   NOT per-app R8 synthesis. R8 *can* outline within
///   io.flutter (its outliner emits at `<host>$$ExternalSyntheticOutline$<id>`
///   per SyntheticNaming.java:104-139); such outlines would show
///   the `$$ExternalSynthetic` infix on the mapping LHS and are
///   filterable in mapping-paired contexts. The bare prefix
///   suppresses both classes of marker in mapping-less sweeps.
///   Anchored empirically by
///   `tests/fixtures/r8/library_helper_rename/`.
/// - `androidx` — AndroidX library namespace, entire. Same
///   structural argument as `io.flutter`: AndroidX ships
///   unminified per AAR (`api` dep), R8 sees and minifies it per
///   app. F-Droid sweep at n=3645 observes the renamed-helper
///   pattern across ~25 androidx subnamespaces (lifecycle,
///   appcompat, activity, core, work, compose, etc.). Canonical
///   AndroidX classes (`androidx.lifecycle.LifecycleRegistry`,
///   `androidx.appcompat.app.AppCompatActivity`, etc.) are the
///   pre-images of the short-name `p`, `j`, etc. we observe; no
///   AndroidX module ships `consumer-rules.pro` that restricts
///   renaming. R8 CAN outline within androidx (the
///   `androidx.work.NetworkType$EnumUnboxingLocalUtility` case is
///   exhibit A — caught by our new SyntheticKind::EnumUnboxing
///   variant); those outlines show `$$ExternalSynthetic` on the
///   mapping LHS and are filterable in mapping-paired contexts.
///   Anchored empirically by
///   `tests/fixtures/r8/androidx_helper_rename/`.
const KNOWN_FP_FAMILY: &[&str] = &[
    "h1",
    "j$",
    "io.flutter",
    "androidx",
];

/// Per-APK namespace rollup buckets emitted into the manifest's
/// `namespace_rollup` column. Stable schema (the row format always
/// lists every bucket, even if count is 0) so downstream tools can
/// parse without column drift. First-match-wins by the same
/// `entry == ns OR ns.starts_with("entry.")` semantics as
/// [`is_in_known_fp_family`]; unbucketed helpers fall into `other`.
///
/// Order:
/// 1. KNOWN_FP_FAMILY entries (h1, j$.com.android.tools.r8, j$,
///    io.flutter, androidx) — so the rollup totals for these match
///    the corpus-level family marker count without any post-hoc
///    reclassification. The `j$.com.android.tools.r8` sub-bucket
///    precedes `j$` for first-match-wins: it isolates D8-remapped
///    R8 runtime helpers (largest single non-family descriptor in
///    the F-Droid corpus) from generic `j$` Java 8+ desugar.
/// 2. Common non-family namespaces frequently seen in F-Droid sweep
///    top-10 (kotlin, kotlinx, com.google, com.android, dagger) —
///    enables namespace-level analysis (e.g., calibration of F2's
///    off-family fraction by ecosystem) without a re-sweep.
///
/// To recompute F2 from a swept manifest: sum the KNOWN_FP_FAMILY
/// bucket counts (h1 + j$.com.android.tools.r8 + j$ + io.flutter
/// + androidx) → family markers; `marker_count - family_markers`
///   → off-family markers.
///   No top-10 truncation involved.
const NAMESPACE_ROLLUP_BUCKETS: &[&str] = &[
    "h1",
    "j$.com.android.tools.r8",
    "j$",
    "io.flutter",
    "androidx",
    "kotlin",
    "kotlinx",
    "com.google",
    "com.android",
    "dagger",
];

/// Returns the [`NAMESPACE_ROLLUP_BUCKETS`] bucket name for a helper
/// class (in dotted FQCN form, e.g. `androidx.work.NetworkType$Foo`),
/// or `"other"` if no bucket matches. First-match-wins.
fn namespace_bucket_for(helper_class: &str) -> &'static str {
    for entry in NAMESPACE_ROLLUP_BUCKETS {
        if helper_class == *entry {
            return entry;
        }
        // SAFETY: format produces a non-empty String; starts_with is byte-safe.
        if helper_class.starts_with(&format!("{entry}.")) {
            return entry;
        }
    }
    "other"
}

/// Falsification criterion thresholds. Identical numeric values to
/// the product-corpus sweep — both sweeps feed the same gauge arm.
const F1_RATE_THRESHOLD_PER_1000: f64 = 2.0;
const F1_MIN_APKS_REQUIRED: usize = 20;
const F2_OFF_FAMILY_FRACTION_THRESHOLD: f64 = 0.30;
const F3_DECOMPILE_FAIL_FRACTION_THRESHOLD: f64 = 0.10;

fn corpus_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("DROIDSAW_R8_FDROID_ROOT")?;
    let p = PathBuf::from(raw);
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

fn sample_n() -> Option<usize> {
    std::env::var("DROIDSAW_R8_FDROID_SAMPLE_N")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
}

/// Shard descriptor parsed from `DROIDSAW_R8_FDROID_SHARD=W/N`.
/// Each worker processes only APKs where the first two hex chars
/// of the sha256 (parsed as u8) modulo N equals W. With 16 workers
/// using `0/16` through `15/16`, every APK is assigned to exactly
/// one worker.
///
/// Returns `None` if unset (single-worker mode). Panics on malformed
/// input — a typo in shard config silently dropping coverage would
/// be the worst kind of bug to debug post-hoc.
fn shard_config() -> Option<(u8, u8)> {
    let raw = std::env::var("DROIDSAW_R8_FDROID_SHARD").ok()?;
    let (w, n) = raw.split_once('/').unwrap_or_else(|| {
        panic!("DROIDSAW_R8_FDROID_SHARD={raw:?}: expected `W/N` (e.g. `0/16`)")
    });
    let w: u8 = w
        .parse()
        .unwrap_or_else(|_| panic!("DROIDSAW_R8_FDROID_SHARD worker_id parse failed: {raw:?}"));
    let n: u8 = n
        .parse()
        .unwrap_or_else(|_| panic!("DROIDSAW_R8_FDROID_SHARD total parse failed: {raw:?}"));
    if n == 0 {
        panic!("DROIDSAW_R8_FDROID_SHARD total must be > 0: {raw:?}");
    }
    if w >= n {
        panic!("DROIDSAW_R8_FDROID_SHARD worker_id ({w}) must be < total ({n})");
    }
    Some((w, n))
}

/// True when this APK falls in the worker's shard. Uses the first
/// hex byte (chars 0..2 of sha256, parsed as u8) modulo N. The
/// first byte is uniformly distributed across `0..=255` so any
/// N ≤ 256 gives a near-uniform shard size.
fn apk_in_shard(sha256: &str, worker_id: u8, total: u8) -> bool {
    let prefix = sha256.get(0..2).unwrap_or("00");
    let byte = u8::from_str_radix(prefix, 16).unwrap_or(0);
    byte % total == worker_id
}

fn droidsaw_sha() -> String {
    let pkg = env!("CARGO_PKG_VERSION");
    let git = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    format!("{pkg}+{git}")
}

fn timestamp_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_iso8601_utc(secs)
}

fn format_iso8601_utc(epoch_secs: u64) -> String {
    let mut days = epoch_secs / 86_400;
    let rem = epoch_secs % 86_400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let mut year: u64 = 1970;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days >= days_in_year {
            days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }
    let months_normal = [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let months_leap = [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let months = if is_leap(year) { months_leap } else { months_normal };
    let mut month = 1u64;
    for &dim in &months {
        if days >= dim {
            days -= dim;
            month += 1;
        } else {
            break;
        }
    }
    let day = days + 1;
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// One row from the F-Droid manifest-latest.tsv.
struct ManifestRow {
    package: String,
    sha256: String,
}

/// Parse the F-Droid manifest. Skips comment lines (starting with `#`).
/// Each row is TAB-separated: package, repo_path, sha256, size, license.
fn read_fdroid_manifest(path: &Path) -> Vec<ManifestRow> {
    let f = match File::open(path) {
        Ok(f) => f,
        Err(e) => panic!("could not open F-Droid manifest at {}: {e}", path.display()),
    };
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(s) => s,
            Err(e) => panic!("read failed on line {line_no} of {}: {e}", path.display()),
        };
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        out.push(ManifestRow {
            package: cols[0].to_string(),
            sha256: cols[2].to_string(),
        });
    }
    out
}

fn apk_blob_path(root: &Path, sha256: &str) -> PathBuf {
    let prefix = &sha256[..2.min(sha256.len())];
    root.join("blobs").join(prefix).join(format!("{sha256}.apk"))
}

fn read_apk_capped(path: &Path) -> Option<Vec<u8>> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("WARN: skipping {} — metadata failed: {e}", path.display());
            return None;
        }
    };
    if meta.file_type().is_symlink() {
        eprintln!("WARN: skipping symlink at {}", path.display());
        return None;
    }
    if meta.len() > MAX_APK_BYTES {
        eprintln!(
            "WARN: skipping {} — {} bytes exceeds {MAX_APK_BYTES}-byte cap",
            path.display(),
            meta.len(),
        );
        return None;
    }
    match std::fs::read(path) {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("WARN: skipping {} — read failed: {e}", path.display());
            None
        }
    }
}

/// Extract every `classes*.dex` entry from an APK zip. Returns
/// Vec<(filename, bytes)>. Skips entries larger than MAX_DEX_BYTES
/// (logged to stderr). Returns empty on zip parse failure.
fn extract_dexes_from_apk(apk_bytes: &[u8], app_id: &str) -> Vec<(String, Vec<u8>)> {
    let cursor = std::io::Cursor::new(apk_bytes);
    let mut zip = match zip::ZipArchive::new(cursor) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("WARN: APK zip-open failed for {app_id}: {e}");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .filter(|n| {
            let bn = n.rsplit('/').next().unwrap_or(n);
            bn.starts_with("classes") && bn.ends_with(".dex")
        })
        .collect();
    for name in names {
        let mut entry = match zip.by_name(&name) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("WARN: APK zip-entry open failed for {app_id} {name}: {e}");
                continue;
            }
        };
        let size = entry.size();
        if size > MAX_DEX_BYTES as u64 {
            eprintln!(
                "WARN: skipping {app_id} {name} — {size} bytes exceeds {MAX_DEX_BYTES}-byte cap",
            );
            continue;
        }
        let mut buf = Vec::with_capacity(size.min(MAX_DEX_BYTES as u64) as usize);
        if let Err(e) = entry.read_to_end(&mut buf) {
            eprintln!("WARN: APK zip-entry read failed for {app_id} {name}: {e}");
            continue;
        }
        out.push((name, buf));
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_in_known_fp_family(mapping_key_class: &str) -> bool {
    KNOWN_FP_FAMILY.iter().any(|entry| {
        mapping_key_class == *entry
            || mapping_key_class.starts_with(&format!("{entry}."))
    })
}

/// Read existing manifest, return set of `apk_sha256` strings
/// already-processed at ANY droidsaw_sha. Used to skip on
/// resumption — the iteration plan favours first-pass coverage
/// over cross-commit re-sweep, so an APK that's been processed
/// once at any prior commit is skipped at this commit too.
///
/// Cross-commit re-sweep (re-process APKs even if covered at a
/// prior commit) is opt-in via `DROIDSAW_R8_FDROID_RESWEEP=1`;
/// when set, this function returns an empty set and the loop
/// processes everything in scope.
fn already_processed(manifest_path: &Path) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    if std::env::var("DROIDSAW_R8_FDROID_RESWEEP").ok().as_deref() == Some("1") {
        return seen;
    }
    let Ok(f) = File::open(manifest_path) else {
        return seen;
    };
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if line.starts_with('#') || line.starts_with("package\t") || line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        // Columns: package, apk_sha256, droidsaw_sha, ...
        seen.insert(cols[1].to_string());
    }
    seen
}

#[derive(Default)]
struct ApkSweep {
    class_count: usize,
    decompile_fail_count: usize,
    marker_count: usize,
    helper_counts: HashMap<String, usize>,
    /// Subset of `helper_counts` restricted to markers whose helper
    /// class name has structural attestation
    /// (`classify_synthetic_kind() != SyntheticKind::Unknown`) — i.e.
    /// the obfuscated name still carries an R8-emitted substring
    /// pattern (`$EnumUnboxingLocalUtility`, `GeneratedOutlineSupport`,
    /// or `$$ExternalSynthetic<kind>`). Used to compute the
    /// `namespace_rollup_attested` column; per-helper key matches
    /// `helper_counts` so per-bucket ratios are computable.
    helper_counts_attested: HashMap<String, usize>,
    off_family_marker_count: usize,
}

#[test]
fn r8_fdroid_apk_sweep() {
    let handle = std::thread::Builder::new()
        .name("r8_fdroid_apk_sweep_worker".into())
        .stack_size(SMOKE_TEST_STACK_BYTES)
        .spawn(sweep_main)
        .expect("spawn stack-sized worker thread");
    if let Err(e) = handle.join() {
        std::panic::resume_unwind(e);
    }
}

fn sweep_main() {
    let Some(root) = corpus_dir() else {
        eprintln!(
            "SKIP: DROIDSAW_R8_FDROID_ROOT unset or does not point at a directory. \
             To sweep the F-Droid APK corpus, set DROIDSAW_R8_FDROID_ROOT to the \
             mirror root containing `index/manifest-latest.tsv` and \
             `blobs/<aa>/<sha256>.apk`. Sample mode via \
             DROIDSAW_R8_FDROID_SAMPLE_N=<N> caps APKs for sanity-test."
        );
        return;
    };

    let index_path = root.join(INDEX_RELATIVE_PATH);
    if !index_path.is_file() {
        eprintln!("SKIP: no F-Droid manifest at {}", index_path.display());
        return;
    }

    let manifest_path = root.join(MANIFEST_FILENAME);
    let mut rows = read_fdroid_manifest(&index_path);
    // Stable sort by sha256 so sample-mode picks the same first-N
    // across runs. Sweep order is otherwise irrelevant.
    rows.sort_by(|a, b| a.sha256.cmp(&b.sha256));

    if let Some(cap) = sample_n() {
        rows.truncate(cap);
        eprintln!(
            "SAMPLE MODE: capped to {} APKs (DROIDSAW_R8_FDROID_SAMPLE_N)",
            rows.len(),
        );
    }

    // Sharded mode: each worker handles only APKs whose sha256[0:2]
    // (first hex byte) modulo N equals its worker_id. Concurrent
    // workers with `0/16` through `15/16` cover the corpus exactly
    // once with no inter-worker coordination beyond append-safe
    // manifest writes.
    if let Some((worker_id, total)) = shard_config() {
        let before = rows.len();
        rows.retain(|r| apk_in_shard(&r.sha256, worker_id, total));
        eprintln!(
            "SHARD MODE: worker {worker_id}/{total} handles {} of {} APKs (sha256 first-byte mod {total})",
            rows.len(),
            before,
        );
    }

    let ds_sha = droidsaw_sha();
    let already = already_processed(&manifest_path);
    let total_in_scope = rows.len();
    let mut already_skip_count = 0usize;
    rows.retain(|r| {
        if already.contains(&r.sha256) {
            already_skip_count += 1;
            false
        } else {
            true
        }
    });
    if already_skip_count > 0 {
        eprintln!(
            "RESUME: skipping {already_skip_count} APKs already in manifest (any droidsaw_sha; first-pass coverage mode). \
             Set DROIDSAW_R8_FDROID_RESWEEP=1 for per-commit re-sweep."
        );
    }

    write_manifest_header_if_new(&manifest_path);

    let mut total_class_count = 0usize;
    let mut total_marker_count = 0usize;
    let mut total_off_family = 0usize;
    let mut apks_swept = 0usize;
    let mut f3_trips: Vec<(String, usize, usize)> = Vec::new();

    for row in &rows {
        let app_id = &row.package;
        let apk_sha = &row.sha256;
        let blob_path = apk_blob_path(&root, apk_sha);
        let Some(apk_bytes) = read_apk_capped(&blob_path) else {
            continue;
        };
        let dexes = extract_dexes_from_apk(&apk_bytes, app_id);
        if dexes.is_empty() {
            eprintln!("{app_id} ({apk_sha}): no classes*.dex in APK — skipping (no row written)");
            continue;
        }

        let mut sweep = ApkSweep::default();
        let mut dex_shas: Vec<String> = Vec::with_capacity(dexes.len());
        for (dex_name, data) in &dexes {
            dex_shas.push(sha256_hex(data));
            let dex = match droidsaw_dex::parser::DexFile::parse(data, None) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("WARN: {app_id} {dex_name} parse failed: {e:?}");
                    continue;
                }
            };
            let census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);
            for class_def in &dex.class_defs {
                if class_def.class_data_off == 0 {
                    continue;
                }
                sweep.class_count = sweep.class_count.saturating_add(1);
                let out = droidsaw_dex::classes::decompile_class_with_census(
                    &dex, data, class_def, &census,
                );
                if out.contains("// Failed to decompile:") {
                    sweep.decompile_fail_count = sweep.decompile_fail_count.saturating_add(1);
                }
                for line in out.lines() {
                    let Some(marker) = parse_block_outlined_marker(line) else {
                        continue;
                    };
                    let key_class = descriptor_to_mapping_key(marker.helper_class);
                    if key_class.is_empty() {
                        continue;
                    }
                    sweep.marker_count = sweep.marker_count.saturating_add(1);
                    let slot = sweep.helper_counts.entry(marker.helper_class.to_string()).or_insert(0);
                    *slot = slot.saturating_add(1);
                    // Structural attestation: if the obfuscated name
                    // still carries an R8-emitted substring pattern
                    // (`$EnumUnboxingLocalUtility`,
                    // `GeneratedOutlineSupport`, or
                    // `$$ExternalSynthetic<kind>`), record it in the
                    // attested set. In mapping-less sweeps most markers
                    // lack attestation (R8 typically strips the
                    // pattern under minification); the attestation
                    // rate per family bucket measures the masquerade
                    // window — see `KNOWN_FP_FAMILY` threat-model
                    // docstring.
                    if classify_synthetic_kind(&key_class) != SyntheticKind::Unknown {
                        let att = sweep
                            .helper_counts_attested
                            .entry(marker.helper_class.to_string())
                            .or_insert(0);
                        *att = att.saturating_add(1);
                    }
                    if !is_in_known_fp_family(&key_class) {
                        sweep.off_family_marker_count =
                            sweep.off_family_marker_count.saturating_add(1);
                    }
                }
            }
        }

        // F3 per-APK check.
        if sweep.class_count > 0 {
            let frac = sweep.decompile_fail_count as f64 / sweep.class_count as f64;
            if frac > F3_DECOMPILE_FAIL_FRACTION_THRESHOLD {
                f3_trips.push((app_id.clone(), sweep.decompile_fail_count, sweep.class_count));
                eprintln!(
                    "F3 TRIPPED on {app_id}: {}/{} = {:.1}% decompile-fail (> {:.0}%)",
                    sweep.decompile_fail_count,
                    sweep.class_count,
                    frac * 100.0,
                    F3_DECOMPILE_FAIL_FRACTION_THRESHOLD * 100.0,
                );
            }
        }

        let mut top10: Vec<(String, usize)> =
            sweep.helper_counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        top10.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        top10.truncate(10);
        let top10_str = top10
            .iter()
            .map(|(d, c)| format!("{d}:{c}"))
            .collect::<Vec<_>>()
            .join(",");

        // Per-APK namespace rollup — every bucket listed in
        // declaration order with explicit zero counts so the schema
        // is positional and stable across rows.
        let mut bucket_counts: std::collections::BTreeMap<&'static str, usize> =
            std::collections::BTreeMap::new();
        let mut bucket_counts_attested: std::collections::BTreeMap<&'static str, usize> =
            std::collections::BTreeMap::new();
        for b in NAMESPACE_ROLLUP_BUCKETS {
            bucket_counts.insert(*b, 0);
            bucket_counts_attested.insert(*b, 0);
        }
        bucket_counts.insert("other", 0);
        bucket_counts_attested.insert("other", 0);
        for (helper, count) in &sweep.helper_counts {
            // Helper class arrives as L/-stripped, dot-separated key.
            let key_class = descriptor_to_mapping_key(helper);
            let bucket = namespace_bucket_for(&key_class);
            *bucket_counts.entry(bucket).or_insert(0) += *count;
        }
        for (helper, count) in &sweep.helper_counts_attested {
            let key_class = descriptor_to_mapping_key(helper);
            let bucket = namespace_bucket_for(&key_class);
            *bucket_counts_attested.entry(bucket).or_insert(0) += *count;
        }
        let mut ns_pairs = Vec::with_capacity(NAMESPACE_ROLLUP_BUCKETS.len() + 1);
        let mut ns_pairs_attested = Vec::with_capacity(NAMESPACE_ROLLUP_BUCKETS.len() + 1);
        for b in NAMESPACE_ROLLUP_BUCKETS {
            ns_pairs.push(format!("{b}={}", bucket_counts.get(b).copied().unwrap_or(0)));
            ns_pairs_attested.push(format!(
                "{b}={}",
                bucket_counts_attested.get(b).copied().unwrap_or(0)
            ));
        }
        ns_pairs.push(format!("other={}", bucket_counts.get("other").copied().unwrap_or(0)));
        ns_pairs_attested.push(format!(
            "other={}",
            bucket_counts_attested.get("other").copied().unwrap_or(0)
        ));
        let ns_str = ns_pairs.join(",");
        let ns_str_attested = ns_pairs_attested.join(",");

        append_manifest_row(
            &manifest_path,
            ManifestRowOut {
                package: app_id,
                apk_sha256: apk_sha,
                droidsaw_sha: &ds_sha,
                timestamp: &timestamp_utc_now(),
                dex_count: dexes.len(),
                dex_sha256_list: &dex_shas.join(","),
                class_count: sweep.class_count,
                decompile_fail_count: sweep.decompile_fail_count,
                marker_count: sweep.marker_count,
                distinct_helper_count: sweep.helper_counts.len(),
                top10_helpers: &top10_str,
                namespace_rollup: &ns_str,
                namespace_rollup_attested: &ns_str_attested,
            },
        );

        eprintln!(
            "{app_id} ({}/{}): {} markers across {} classes ({} DEXes, {} decompile-fails)",
            apks_swept + 1,
            rows.len(),
            sweep.marker_count,
            sweep.class_count,
            dexes.len(),
            sweep.decompile_fail_count,
        );

        total_class_count = total_class_count.saturating_add(sweep.class_count);
        total_marker_count = total_marker_count.saturating_add(sweep.marker_count);
        total_off_family = total_off_family.saturating_add(sweep.off_family_marker_count);
        apks_swept = apks_swept.saturating_add(1);
    }

    eprintln!(
        "---\nAGGREGATE: {} APKs swept ({} skipped from resume; {} in scope), {} classes, {} markers, {} markers off-family",
        apks_swept,
        already_skip_count,
        total_in_scope,
        total_class_count,
        total_marker_count,
        total_off_family,
    );

    // F1 aggregate check.
    if total_class_count > 0 {
        let rate_per_1000 = (total_marker_count as f64 / total_class_count as f64) * 1000.0;
        if apks_swept >= F1_MIN_APKS_REQUIRED && rate_per_1000 > F1_RATE_THRESHOLD_PER_1000 {
            eprintln!(
                "F1 TRIPPED: {} markers / {} classes = {:.2}/1000 (> {:.0}/1000), apks_swept={}",
                total_marker_count, total_class_count, rate_per_1000, F1_RATE_THRESHOLD_PER_1000, apks_swept,
            );
        } else if apks_swept < F1_MIN_APKS_REQUIRED {
            eprintln!(
                "F1 INSUFFICIENT SAMPLE: {} markers / {} classes = {:.2}/1000, apks_swept={} (need >= {})",
                total_marker_count, total_class_count, rate_per_1000, apks_swept, F1_MIN_APKS_REQUIRED,
            );
        } else {
            eprintln!(
                "F1 ok: {} markers / {} classes = {:.2}/1000 (<= {:.0}/1000)",
                total_marker_count, total_class_count, rate_per_1000, F1_RATE_THRESHOLD_PER_1000,
            );
        }
    }

    // F2 aggregate check.
    if total_marker_count > 0 {
        let frac = total_off_family as f64 / total_marker_count as f64;
        if frac > F2_OFF_FAMILY_FRACTION_THRESHOLD {
            eprintln!(
                "F2 TRIPPED: {}/{} markers off known-FP family (> {:.0}%)",
                total_off_family,
                total_marker_count,
                F2_OFF_FAMILY_FRACTION_THRESHOLD * 100.0,
            );
        } else {
            eprintln!(
                "F2 ok: {}/{} markers off known-FP family (<= {:.0}%)",
                total_off_family,
                total_marker_count,
                F2_OFF_FAMILY_FRACTION_THRESHOLD * 100.0,
            );
        }
    }

    if !f3_trips.is_empty() {
        eprintln!("F3 trips ({} APKs):", f3_trips.len());
        for (app_id, fail, total) in &f3_trips {
            eprintln!("  {app_id}: {fail}/{total} decompile failures");
        }
    } else {
        eprintln!("F3 ok: no per-APK trips");
    }
}

struct ManifestRowOut<'a> {
    package: &'a str,
    apk_sha256: &'a str,
    droidsaw_sha: &'a str,
    timestamp: &'a str,
    dex_count: usize,
    dex_sha256_list: &'a str,
    class_count: usize,
    decompile_fail_count: usize,
    marker_count: usize,
    distinct_helper_count: usize,
    top10_helpers: &'a str,
    namespace_rollup: &'a str,
    namespace_rollup_attested: &'a str,
}

fn write_manifest_header_if_new(manifest_path: &Path) {
    if manifest_path.exists() {
        return;
    }
    let header = "# Each row is one F-Droid APK sweep at one droidsaw commit.\n\
        # Columns are tab-separated. dex_sha256_list is comma-separated.\n\
        # top10_helpers is comma-separated <descriptor>:<count> pairs.\n\
        # namespace_rollup is comma-separated <bucket>=<count> pairs in\n\
        # a fixed positional order: h1, j$, io.flutter, androidx, kotlin,\n\
        # kotlinx, com.google, com.android, dagger, other. Zero buckets\n\
        # are emitted explicitly so the schema is stable across rows.\n\
        # Sum of bucket counts equals marker_count.\n\
        # namespace_rollup_attested mirrors namespace_rollup (same bucket\n\
        # order, same zero-explicit discipline) but restricts to markers\n\
        # whose helper class name carries an R8-emitted structural\n\
        # substring pattern ($EnumUnboxingLocalUtility,\n\
        # GeneratedOutlineSupport, or $$ExternalSynthetic<kind>). The\n\
        # ratio attested[bucket] / total[bucket] measures the\n\
        # attestation rate per bucket — low rates signal a wide\n\
        # masquerade window in mapping-less analysis. See\n\
        # KNOWN_FP_FAMILY docstring for threat model.\n\
        package\tapk_sha256\tdroidsaw_sha\ttimestamp_utc\tdex_count\tdex_sha256_list\tclass_count\tdecompile_fail_count\tmarker_count\tdistinct_helper_count\ttop10_helpers\tnamespace_rollup\tnamespace_rollup_attested\n";
    if let Err(e) = std::fs::write(manifest_path, header) {
        panic!("failed to write manifest header to {}: {e}", manifest_path.display());
    }
}

fn append_manifest_row(manifest_path: &Path, row: ManifestRowOut<'_>) {
    let mut f = match OpenOptions::new().append(true).open(manifest_path) {
        Ok(f) => f,
        Err(e) => panic!(
            "failed to open manifest {} for append: {e}",
            manifest_path.display(),
        ),
    };
    // Sanitize package name against TSV-corrupting whitespace.
    if row.package.contains('\t') || row.package.contains('\n') || row.package.contains('\r') {
        panic!(
            "package id contains TSV-corrupting whitespace: {:?}",
            row.package,
        );
    }
    let line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        row.package,
        row.apk_sha256,
        row.droidsaw_sha,
        row.timestamp,
        row.dex_count,
        row.dex_sha256_list,
        row.class_count,
        row.decompile_fail_count,
        row.marker_count,
        row.distinct_helper_count,
        row.top10_helpers,
        row.namespace_rollup,
        row.namespace_rollup_attested,
    );
    if let Err(e) = f.write_all(line.as_bytes()) {
        panic!(
            "failed to write manifest row to {}: {e}",
            manifest_path.display(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_basic() {
        assert_eq!(format_iso8601_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_fp_family_membership() {
        // Bare-name + dot-prefix entries match canonical R8/D8
        // emit shapes; word-boundary distractors do not.
        assert!(is_in_known_fp_family("h1"));
        // j$ — entire D8 desugared-library namespace per
        // LibraryDesugaredChecker.java:81 (startsWith "Lj$/").
        assert!(is_in_known_fp_family("j$"));
        assert!(is_in_known_fp_family("j$.time"));
        assert!(is_in_known_fp_family("j$.time.b"));
        assert!(is_in_known_fp_family("j$.time.chrono.i"));
        assert!(is_in_known_fp_family("j$.util"));
        assert!(is_in_known_fp_family("j$.util.DesugarArrays"));
        assert!(is_in_known_fp_family("j$.util.DesugarArrays.foo"));
        assert!(is_in_known_fp_family("j$.util.Optional"));
        assert!(is_in_known_fp_family("j$.util.stream.Collectors"));
        assert!(is_in_known_fp_family("j$.com.android.tools.r8.DesugarVarHandle"));
        // io.flutter — Flutter engine namespace; R8-renamed via
        // -allowshrinking,-allowobfuscation keep rule.
        assert!(is_in_known_fp_family("io.flutter"));
        assert!(is_in_known_fp_family("io.flutter.plugin.platform.i"));
        assert!(is_in_known_fp_family("io.flutter.embedding.engine.a"));
        // androidx — entire library namespace; per-AAR `api` dep
        // minified per-app, no surface-preserving keep rules.
        assert!(is_in_known_fp_family("androidx"));
        assert!(is_in_known_fp_family("androidx.lifecycle.LifecycleRegistry"));
        assert!(is_in_known_fp_family("androidx.appcompat.app.h"));
        assert!(is_in_known_fp_family("androidx.work.impl.utils.j"));
        // Word-boundary distractors — not family.
        assert!(!is_in_known_fp_family("h10"));
        assert!(!is_in_known_fp_family("j$utilities"));
        assert!(!is_in_known_fp_family("io.flutterfoo"));
        assert!(!is_in_known_fp_family("androidxfoo"));
        assert!(!is_in_known_fp_family("com.example.MyClass"));
    }

    #[test]
    fn apk_blob_path_uses_first_two_chars() {
        let root = Path::new("/x");
        let p = apk_blob_path(root, "ab12cdef");
        assert_eq!(p, Path::new("/x/blobs/ab/ab12cdef.apk"));
    }

    #[test]
    fn sha256_hex_known() {
        // sha256 of "abc"
        let h = sha256_hex(b"abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
    }

    #[test]
    fn shard_partitions_corpus_exactly_once() {
        // For any sha256 first-hex-byte 0..=0xff, with N=16, each
        // byte modulo 16 falls in exactly one worker_id 0..=15.
        let total: u8 = 16;
        let mut covered: [usize; 16] = [0; 16];
        for byte in 0u16..=0xff {
            let sha = format!("{byte:02x}0000000000000000");
            let mut hits = 0;
            for w in 0..total {
                if apk_in_shard(&sha, w, total) {
                    hits += 1;
                    covered[w as usize] += 1;
                }
            }
            assert_eq!(hits, 1, "sha {sha} matched {hits} shards, expected 1");
        }
        // Each worker should see exactly 256/16 = 16 distinct first bytes.
        for (i, &c) in covered.iter().enumerate() {
            assert_eq!(c, 16, "worker {i} covered {c} bytes, expected 16");
        }
    }

    #[test]
    fn shard_assignment_examples() {
        // Specific assignments to pin behaviour against documentation.
        assert!(apk_in_shard("00abcdef", 0, 16));
        assert!(!apk_in_shard("00abcdef", 1, 16));
        assert!(apk_in_shard("0fabcdef", 15, 16));
        assert!(apk_in_shard("10abcdef", 0, 16));
        assert!(apk_in_shard("ffabcdef", 15, 16));
    }

    #[test]
    fn namespace_bucket_matches_known_family_and_common_ecosystems() {
        // KNOWN_FP_FAMILY entries bucket to themselves.
        assert_eq!(namespace_bucket_for("h1"), "h1");
        assert_eq!(namespace_bucket_for("h1.ext"), "h1");
        assert_eq!(namespace_bucket_for("j$"), "j$");
        assert_eq!(namespace_bucket_for("j$.time.Instant"), "j$");
        // Sub-bucket (precedes "j$" for first-match-wins).
        assert_eq!(
            namespace_bucket_for("j$.com.android.tools.r8.a"),
            "j$.com.android.tools.r8",
        );
        assert_eq!(
            namespace_bucket_for("j$.com.android.tools.r8.deeper.sub"),
            "j$.com.android.tools.r8",
        );
        assert_eq!(namespace_bucket_for("io.flutter.plugin.platform.i"), "io.flutter");
        assert_eq!(namespace_bucket_for("androidx.work.NetworkType$EnumUnboxingLocalUtility"), "androidx");

        // Common non-family ecosystem buckets.
        assert_eq!(namespace_bucket_for("kotlin.collections.AbstractList"), "kotlin");
        assert_eq!(namespace_bucket_for("kotlinx.coroutines.flow.FlowKt"), "kotlinx");
        assert_eq!(namespace_bucket_for("com.google.common.collect.ImmutableList"), "com.google");
        assert_eq!(namespace_bucket_for("com.android.tools.r8.Foo"), "com.android");
        assert_eq!(namespace_bucket_for("dagger.internal.Provider"), "dagger");

        // Word-boundary discipline (no false-positive on prefix-shaped lookalikes).
        assert_eq!(namespace_bucket_for("h10.ext"), "other");
        assert_eq!(namespace_bucket_for("io.flutterfoo.Bar"), "other");
        assert_eq!(namespace_bucket_for("androidxfoo.bar"), "other");
        assert_eq!(namespace_bucket_for("kotlinish.Foo"), "other");

        // Unbucketed third-party.
        assert_eq!(namespace_bucket_for("okhttp3.Request"), "other");
        assert_eq!(namespace_bucket_for("com.example.MyClass"), "other");
        assert_eq!(namespace_bucket_for("org.slf4j.event.Level"), "other");
    }

    #[test]
    fn namespace_bucket_first_match_wins_for_disjoint_prefixes() {
        // androidx must not be shadowed by com.android (disjoint top-level).
        assert_eq!(namespace_bucket_for("androidx.lifecycle.LifecycleRegistry"), "androidx");
        // com.android.tools is com.android, not androidx.
        assert_eq!(namespace_bucket_for("com.android.tools.r8.Foo"), "com.android");
    }
}
