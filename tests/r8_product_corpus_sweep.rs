//! Env-gated per-APK sweep harness for the R8 BlockOutlined recogniser.
//!
//! Falsification criteria F1/F2/F3 are pre-registered numeric thresholds;
//! the sweep produces aggregate calibration data feeding the FP-ceiling
//! arm of the validation gauge.
//!
//! This is a **mapping-less** sweep: closed-source product APKs ship
//! without `mapping.txt`. Per-individual-marker TP/FP labels are
//! unavailable; the sweep produces aggregate calibration data feeding
//! the FP-ceiling arm of the validation gauge.
//!
//! Reads `$DROIDSAW_R8_PRODUCT_ROOT` (a directory). Each immediate
//! subdirectory is treated as one APK boundary; the subdirectory name
//! is the `app_id`. Appends one row per APK to
//! `$DROIDSAW_R8_PRODUCT_ROOT/swept-manifest.tsv`.
//!
//! Skips cleanly when the env var is unset or does not point to a
//! directory.
//!
//! # Adversarial-input discipline
//!
//! Same defences as `r8_production_corpus_smoke`:
//! - Symlink reject on every directory entry and per-file open.
//! - 64 MiB per-DEX size cap.
//! - 16 MiB worker thread stack.
//! - F1/F2/F3 trips are findings logged to stderr + the manifest;
//!   they do NOT panic the test (the sweep is exploratory by design).
//! - I/O failures writing the manifest, mid-walk parser errors, and
//!   clock backward panics DO trip — they invalidate the data.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod common;

use common::r8_canonical_marker::{
    descriptor_to_mapping_key, parse_block_outlined_marker,
};

const MAX_DEX_BYTES: u64 = 64 * 1024 * 1024;
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;
const MANIFEST_FILENAME: &str = "swept-manifest.tsv";

/// Known false-positive descriptor families observed on the
/// mapping-paired ratchet. Membership matches by EXACT
/// mapping-key class string OR prefix-followed-by-`.` (so
/// `j$.util.DesugarArrays` matches `j$.util.DesugarArrays` itself and
/// any inner-classed `j$.util.DesugarArrays.Foo`).
///
/// Adding members requires a structural justification — cargo-cult
/// additions are out. All members must be known false positives with
/// documented reasoning.
const KNOWN_FP_FAMILY: &[&str] = &[
    "h1",                    // horizontal-merge bridge (R8 synthesized, not outline-annotated)
    "j$.time",               // D8 desugar (java.time backport)
    "j$.util.DesugarArrays", // D8 desugar (java.util.Arrays backport)
];

/// F1 — corpus-wide marker rate trip (markers/class). 2/1000.
const F1_RATE_THRESHOLD_NUM: usize = 2;
const F1_RATE_THRESHOLD_DEN: usize = 1000;
const F1_MIN_APKS: usize = 20;

/// F2 — fraction of total markers landing OUTSIDE the known-FP
/// family. 30%.
const F2_NUMERATOR: usize = 30;
const F2_DENOMINATOR: usize = 100;

/// F3 — per-APK decompile-failure rate. 10%.
const F3_NUMERATOR: usize = 10;
const F3_DENOMINATOR: usize = 100;

fn corpus_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("DROIDSAW_R8_PRODUCT_ROOT")?;
    let p = PathBuf::from(raw);
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

/// Walk `root` and return `.dex` paths. Symlink reject mirrors the
/// production-corpus smoke test discipline.
fn collect_dex_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let entry = entry?;
            let p = entry.path();
            let meta = std::fs::symlink_metadata(&p)?;
            if meta.file_type().is_symlink() {
                eprintln!("WARN: skipping symlink at {}", p.display());
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("dex") {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// List immediate subdirectories of `root` (each is one APK boundary).
/// Symlinks are rejected — the same discipline applied to DEX walking.
fn collect_apk_dirs(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let p = entry.path();
        let meta = std::fs::symlink_metadata(&p)?;
        if meta.file_type().is_symlink() {
            eprintln!("WARN: skipping symlinked APK dir at {}", p.display());
            continue;
        }
        if meta.is_dir() {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

fn read_dex_capped(path: &Path) -> Option<Vec<u8>> {
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
    if meta.len() > MAX_DEX_BYTES {
        eprintln!(
            "WARN: skipping {} — {} bytes exceeds {MAX_DEX_BYTES}-byte cap",
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

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        // Local hex encoding — avoids a hex-crate dep.
        const HEX: &[u8; 16] = b"0123456789abcdef";
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Capture droidsaw's identity at sweep time. Returns
/// `<pkg-version>+<git-sha-or-unknown>` so a row can be replayed at
/// the same recogniser version.
fn droidsaw_sha() -> String {
    let pkg = env!("CARGO_PKG_VERSION");
    let git = match Command::new("git").args(["rev-parse", "HEAD"]).output() {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_owned();
            if s.is_empty() {
                "unknown".to_owned()
            } else {
                s
            }
        }
        _ => "unknown".to_owned(),
    };
    format!("{pkg}+{git}")
}

/// ISO 8601 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`) derived from
/// `SystemTime::now()`. Inline rather than a `chrono` dep — the
/// manifest doesn't require sub-second precision or local-tz handling,
/// and the test's Cargo.toml does not currently list chrono.
///
/// Panics if the system clock returns a duration BEFORE the Unix
/// epoch — that invalidates the manifest's `timestamp_utc` ordering
/// guarantee, which is exactly the case the brief calls out as
/// panic-on.
fn timestamp_utc_now() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH — manifest ordering would be invalid");
    format_iso8601_utc(dur.as_secs())
}

/// Pure function: civil-time conversion of a Unix-epoch second count
/// to `YYYY-MM-DDTHH:MM:SSZ`. Algorithm is the standard days-since-
/// 1970 walk over the Gregorian calendar (366 day Feb-29 in every
/// year divisible by 4 except centennials not divisible by 400). Pure
/// integer arithmetic — no floating point, no tz database lookup.
fn format_iso8601_utc(secs: u64) -> String {
    let day_secs: u64 = 86_400;
    let mut days = secs / day_secs;
    let tod = secs % day_secs;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;

    let mut year: u64 = 1970;
    loop {
        let dy: u64 = if is_leap(year) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let mdays: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month: usize = 0;
    while month < 12 && days >= mdays[month] {
        days -= mdays[month];
        month += 1;
    }
    let day_of_month = days + 1;
    format!(
        "{year:04}-{m:02}-{d:02}T{h:02}:{mn:02}:{s:02}Z",
        m = month + 1,
        d = day_of_month,
        h = hour,
        mn = min,
        s = sec,
    )
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// Reject `app_id` strings that would corrupt the TSV manifest's
/// one-row-per-APK invariant. Unix filesystems permit tab,
/// newline, and carriage-return in directory names — any of these
/// in the column 1 cell would split the row across multiple lines
/// or shift downstream columns. Returns the reason on rejection.
fn validate_app_id(s: &str) -> Result<(), &'static str> {
    if s.contains('\t') {
        return Err("contains tab");
    }
    if s.contains('\n') {
        return Err("contains newline");
    }
    if s.contains('\r') {
        return Err("contains carriage return");
    }
    Ok(())
}

/// Per-APK accumulated counts for one sweep iteration. The `app_id`
/// is kept as a local in the caller; this struct is the
/// numeric/aggregate slice only.
#[derive(Default)]
struct ApkSweep {
    dex_sha256s: Vec<String>,
    class_count: usize,
    decompile_fail_count: usize,
    marker_count: usize,
    /// helper_class (DEX descriptor `L...;`) -> firing count.
    helper_counts: HashMap<String, usize>,
    /// Markers landing outside the known-FP family (mapped via
    /// `descriptor_to_mapping_key` + prefix-or-exact match).
    markers_off_family: usize,
}

/// Membership test for the known-FP family. Matches by exact
/// mapping-key class OR by `<entry>.` prefix.
fn is_in_known_fp_family(mapping_key: &str) -> bool {
    if mapping_key.is_empty() {
        return false;
    }
    for entry in KNOWN_FP_FAMILY {
        if mapping_key == *entry {
            return true;
        }
        // Use a literal `.` separator so `h1` does NOT match `h10`.
        let with_dot = format!("{entry}.");
        if mapping_key.starts_with(&with_dot) {
            return true;
        }
    }
    false
}

#[test]
fn r8_product_corpus_sweep() {
    let handle = std::thread::Builder::new()
        .name("r8_product_corpus_sweep_worker".into())
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
            "SKIP: DROIDSAW_R8_PRODUCT_ROOT unset or does not point at a directory. \
             To run the per-APK sweep of the R8 BlockOutlined recogniser, point \
             this env var at a directory whose immediate subdirectories each \
             contain the `.dex` files extracted from one APK (subdirectory \
             name = app_id). The sweep appends one row per APK to \
             $DROIDSAW_R8_PRODUCT_ROOT/swept-manifest.tsv. Measurement-only — does not gate CI.",
        );
        return;
    };

    let apks = match collect_apk_dirs(&root) {
        Ok(v) => v,
        Err(e) => {
            // I/O failure walking the root is data-invalidating —
            // panic per the brief's "panic on I/O error" discipline.
            panic!("failed to list APK subdirectories under {}: {e}", root.display());
        }
    };
    if apks.is_empty() {
        eprintln!(
            "SKIP: no subdirectories under {}. Each immediate subdirectory is one APK; \
             add at least one and re-run.",
            root.display(),
        );
        return;
    }

    let ds_sha = droidsaw_sha();
    let manifest_path = root.join(MANIFEST_FILENAME);
    let manifest_exists = manifest_path.exists();
    let mut manifest = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&manifest_path)
        .unwrap_or_else(|e| {
            panic!(
                "failed to open manifest {} for append: {e}",
                manifest_path.display(),
            )
        });
    if !manifest_exists {
        // Header is written once on file creation; subsequent runs
        // append data rows only.
        let header = "# Each row is one APK sweep at one droidsaw commit.\n\
                      # Columns are tab-separated. dex_sha256_list is comma-separated.\n\
                      # top10_helpers is comma-separated <descriptor>:<count> pairs.\n\
                      app_id\tdroidsaw_sha\ttimestamp_utc\tdex_count\tdex_sha256_list\tclass_count\tdecompile_fail_count\tmarker_count\tdistinct_helper_count\ttop10_helpers\n";
        manifest
            .write_all(header.as_bytes())
            .unwrap_or_else(|e| panic!("failed to write manifest header: {e}"));
    }

    // Corpus-wide accumulators for the F1/F2 aggregate trip checks.
    let mut total_classes: usize = 0;
    let mut total_markers: usize = 0;
    let mut total_off_family: usize = 0;
    let mut apks_swept: usize = 0;
    let mut f3_trips: Vec<(String, usize, usize)> = Vec::new();

    for apk_dir in &apks {
        let app_id = apk_dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<unnamed>".to_owned());
        if let Err(reason) = validate_app_id(&app_id) {
            panic!(
                "app_id rejected ({reason}): {app_id:?}. Rename the offending \
                 subdirectory under DROIDSAW_R8_PRODUCT_ROOT and rerun.",
            );
        }

        let dexes = match collect_dex_files(apk_dir) {
            Ok(v) => v,
            Err(e) => {
                // Mid-walk I/O failure invalidates the row — panic
                // per the brief's "panic on mid-walk parser/IO error".
                panic!(
                    "failed to walk DEXes under {} (app_id={app_id}): {e}",
                    apk_dir.display(),
                );
            }
        };
        if dexes.is_empty() {
            eprintln!("{app_id}: no DEXes — skipping (no row written)");
            continue;
        }

        let mut sweep = ApkSweep::default();

        for dex_path in &dexes {
            let Some(data) = read_dex_capped(dex_path) else {
                // Defended skip (symlink / cap / read error). The
                // file was rejected for safety; not a parser bug.
                continue;
            };
            sweep.dex_sha256s.push(sha256_hex(&data));

            let dex = match droidsaw_dex::parser::DexFile::parse(&data, None) {
                Ok(d) => d,
                Err(e) => {
                    // Parser errors on a real DEX in the corpus are
                    // data-invalidating mid-walk events — panic per
                    // the brief.
                    panic!(
                        "parser failed on {} (app_id={app_id}): {e:?}",
                        dex_path.display(),
                    );
                }
            };

            let r8_census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);
            for class_def in &dex.class_defs {
                sweep.class_count = sweep.class_count.saturating_add(1);
                if class_def.class_data_off == 0 {
                    continue;
                }
                let out = droidsaw_dex::classes::decompile_class_with_census(
                    &dex, &data, class_def, &r8_census,
                );
                if out.contains("// Failed to decompile:") {
                    sweep.decompile_fail_count = sweep.decompile_fail_count.saturating_add(1);
                }
                for line in out.lines() {
                    let Some(m) = parse_block_outlined_marker(line) else {
                        continue;
                    };
                    sweep.marker_count = sweep.marker_count.saturating_add(1);
                    *sweep
                        .helper_counts
                        .entry(m.helper_class.to_owned())
                        .or_insert(0) += 1;
                    let key = descriptor_to_mapping_key(m.helper_class);
                    if !is_in_known_fp_family(&key) {
                        sweep.markers_off_family = sweep.markers_off_family.saturating_add(1);
                    }
                }
            }
        }

        // Per-APK summary to stderr.
        eprintln!(
            "{app_id}: {} markers across {} classes ({} DEXes, {} decompile-fails)",
            sweep.marker_count,
            sweep.class_count,
            sweep.dex_sha256s.len(),
            sweep.decompile_fail_count,
        );

        // F3 per-APK trip check: log if decompile-fail rate exceeds 10%.
        // Avoid division — multiply across. `class_count == 0` is
        // possible on an empty-class DEX; treat that as "no trip".
        if sweep.class_count > 0
            && sweep
                .decompile_fail_count
                .saturating_mul(F3_DENOMINATOR)
                > sweep.class_count.saturating_mul(F3_NUMERATOR)
        {
            eprintln!(
                "F3 TRIPPED on {app_id}: decompile_fail={}/{} ({}%)",
                sweep.decompile_fail_count,
                sweep.class_count,
                sweep
                    .decompile_fail_count
                    .saturating_mul(100)
                    .checked_div(sweep.class_count)
                    .unwrap_or(0),
            );
            f3_trips.push((
                app_id.clone(),
                sweep.decompile_fail_count,
                sweep.class_count,
            ));
        }

        // Build top10_helpers (descending by count, max 10).
        let mut helper_pairs: Vec<(String, usize)> =
            sweep.helper_counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        helper_pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let top10: Vec<String> = helper_pairs
            .iter()
            .take(10)
            .map(|(k, v)| format!("{k}:{v}"))
            .collect();
        let top10_str = top10.join(",");
        let dex_sha256_list = sweep.dex_sha256s.join(",");
        let distinct_helper_count = sweep.helper_counts.len();
        let timestamp = timestamp_utc_now();

        // Append the row. I/O failure here invalidates the manifest
        // discipline (silent partial writes) — panic.
        let row = format!(
            "{app_id}\t{ds_sha}\t{timestamp}\t{dex_count}\t{dex_sha256_list}\t{class_count}\t{decompile_fail_count}\t{marker_count}\t{distinct_helper_count}\t{top10_str}\n",
            dex_count = sweep.dex_sha256s.len(),
            class_count = sweep.class_count,
            decompile_fail_count = sweep.decompile_fail_count,
            marker_count = sweep.marker_count,
        );
        manifest
            .write_all(row.as_bytes())
            .unwrap_or_else(|e| panic!("failed to write manifest row for {app_id}: {e}"));

        total_classes = total_classes.saturating_add(sweep.class_count);
        total_markers = total_markers.saturating_add(sweep.marker_count);
        total_off_family = total_off_family.saturating_add(sweep.markers_off_family);
        apks_swept = apks_swept.saturating_add(1);
    }

    // Flush before the aggregate so a downstream `tail -f` consumer
    // sees the full row set before the summary block.
    manifest
        .flush()
        .unwrap_or_else(|e| panic!("failed to flush manifest: {e}"));

    // Aggregate F1/F2 checks. F3 is per-APK and already reported above.
    eprintln!("---");
    eprintln!(
        "AGGREGATE: {apks_swept} APKs swept, {total_classes} classes, \
         {total_markers} markers, {total_off_family} markers off-family",
    );

    // F1: marker rate > 2/1000 AND apks_swept >= 20.
    // Cross-multiplied: total_markers * 1000 > 2 * total_classes.
    let f1_lhs = total_markers.saturating_mul(F1_RATE_THRESHOLD_DEN);
    let f1_rhs = total_classes.saturating_mul(F1_RATE_THRESHOLD_NUM);
    if total_classes > 0 && apks_swept >= F1_MIN_APKS && f1_lhs > f1_rhs {
        eprintln!(
            "F1 TRIPPED: {total_markers} markers / {total_classes} classes \
             > {F1_RATE_THRESHOLD_NUM}/{F1_RATE_THRESHOLD_DEN} (apks_swept={apks_swept})",
        );
    } else {
        eprintln!(
            "F1 ok: {total_markers} markers / {total_classes} classes \
             (need apks_swept >= {F1_MIN_APKS}, have {apks_swept})",
        );
    }

    // F2: off-family fraction > 30% when total_markers > 0.
    // Cross-multiplied: off_family * 100 > 30 * total_markers.
    if total_markers > 0 {
        let f2_lhs = total_off_family.saturating_mul(F2_DENOMINATOR);
        let f2_rhs = total_markers.saturating_mul(F2_NUMERATOR);
        if f2_lhs > f2_rhs {
            eprintln!(
                "F2 TRIPPED: {total_off_family}/{total_markers} markers off known-FP family \
                 (> {F2_NUMERATOR}%)",
            );
        } else {
            eprintln!(
                "F2 ok: {total_off_family}/{total_markers} markers off known-FP family",
            );
        }
    } else {
        eprintln!("F2 ok: no markers fired (total_markers=0)");
    }

    if f3_trips.is_empty() {
        eprintln!("F3 ok: no per-APK trips");
    } else {
        eprintln!("F3 trips: {} APKs", f3_trips.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_epoch_is_well_formed() {
        assert_eq!(format_iso8601_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn validate_app_id_accepts_normal_names() {
        assert!(validate_app_id("airbnb-re").is_ok());
        assert!(validate_app_id("amazon.alexa-re").is_ok());
        assert!(validate_app_id("chase-re_v2").is_ok());
        assert!(validate_app_id("foo bar baz").is_ok(), "regular spaces ok");
    }

    #[test]
    fn validate_app_id_rejects_tsv_corrupting_whitespace() {
        // Tab / newline / carriage-return would split a single APK
        // row across multiple manifest rows or columns.
        assert!(validate_app_id("foo\tbar").is_err());
        assert!(validate_app_id("foo\nbar").is_err());
        assert!(validate_app_id("foo\rbar").is_err());
        assert!(validate_app_id("trailing\n").is_err());
        assert!(validate_app_id("\ttab-prefix").is_err());
    }

    #[test]
    fn iso8601_known_date_2026_05_23() {
        // 2026-05-23T00:00:00Z corresponds to days-since-epoch:
        //   1970..2026 inclusive of 14 leap years (1972,76,80,84,88,
        //   92,96,2000,04,08,12,16,20,24) -> 14 leaps.
        //   (2026-1970)*365 + 14 = 56*365 + 14 = 20440 + 14 = 20454.
        //   Plus 31 (Jan) + 28 (Feb 2026 non-leap) + 31 (Mar) +
        //        30 (Apr) + 22 (days into May before 23rd) = 142.
        //   Total: 20454 + 142 = 20596 days = 20596 * 86400 seconds.
        let secs = 20596u64 * 86_400;
        assert_eq!(format_iso8601_utc(secs), "2026-05-23T00:00:00Z");
    }

    #[test]
    fn iso8601_leap_day_2024_02_29() {
        // 2024 is a leap year. Feb 29 2024:
        //   (2024-1970)*365 + 13 leaps (1972..2020 inclusive every 4
        //     = 13 leaps) = 54*365 + 13 = 19710 + 13 = 19723.
        //   Plus 31 (Jan) + 28 (Feb 1..28) = 59 -> day-index 59 is
        //   Feb 29 (zero-based).
        let secs = (19723u64 + 59) * 86_400;
        assert_eq!(format_iso8601_utc(secs), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn known_fp_family_exact_match() {
        assert!(is_in_known_fp_family("h1"));
        assert!(is_in_known_fp_family("j$.time"));
        assert!(is_in_known_fp_family("j$.util.DesugarArrays"));
    }

    #[test]
    fn known_fp_family_prefix_match_with_dot() {
        assert!(is_in_known_fp_family("j$.time.LocalDate"));
        assert!(is_in_known_fp_family("j$.util.DesugarArrays.someInner"));
    }

    #[test]
    fn known_fp_family_rejects_prefix_collision() {
        // `h1` must not match `h10`, `h11`, `h1x` etc. — the prefix
        // gate uses a literal `.` separator.
        assert!(!is_in_known_fp_family("h10"));
        assert!(!is_in_known_fp_family("h11"));
        assert!(!is_in_known_fp_family("h1x"));
    }

    #[test]
    fn known_fp_family_rejects_unknown() {
        assert!(!is_in_known_fp_family("a"));
        assert!(!is_in_known_fp_family("com.example.Foo"));
        assert!(!is_in_known_fp_family(""));
    }

    #[test]
    fn sha256_hex_matches_known_value() {
        // sha256("") is well-known.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }
}
