//! Env-gated mapping-paired sweep across a curated corpus of
//! (APK, mapping.txt) pairs. Sibling to [`r8_fdroid_apk_sweep`].
//!
//! Where the F-Droid sweep produces mapping-LESS aggregate
//! calibration data (every TP/FP claim is inferred from structural
//! gates), this harness produces mapping-PAIRED ground-truth
//! per-variant TP/FP/FN per APK. The corpus is a set of
//! (APK, mapping.txt) tuples curated from open-source projects that
//! publish their R8 mapping artifacts alongside release APKs (Mozilla
//! Firefox / Focus, Signal, Bitwarden, ProtonMail, Mullvad VPN,
//! WireGuard, Briar, etc.).
//!
//! # Layout
//!
//! Reads `$DROIDSAW_R8_PAIRED_ROOT` (the corpus root, containing
//! `index/manifest-pairs.tsv` + `blobs/<slot>/app.apk` +
//! `blobs/<slot>/mapping.txt` + `blobs/<slot>/provenance.toml`).
//!
//! Per design, per-app subdirectories use opaque slot identifiers
//! (alphanumeric, no vendor names in checked-in code or filenames).
//! `provenance.toml` per slot carries the vendor identity + public
//! source URL.
//!
//! Appends one row per pair to `$ROOT/swept-manifest-paired.tsv`.
//!
//! # Output schema
//!
//! Columns are tab-separated:
//!   slot, apk_sha256, mapping_sha256, droidsaw_sha, timestamp_utc,
//!   r8_version, dex_count, class_count, marker_count,
//!   outlined_in_mapping, tp_count, fp_count, fn_count,
//!   per_variant_breakdown
//!
//! `per_variant_breakdown` is `;`-separated `<kind>:total=N,tp=M` per
//! SyntheticKind in report order. `total` is outline annotations of
//! that kind in mapping; `tp` is markers that fired AND matched a
//! mapping annotation of that kind.
//!
//! # Adversarial-input discipline
//!
//! - Symlink reject on every blob + mapping read.
//! - 64 MiB per-DEX size cap when extracting from the APK zip.
//! - 256 MiB per-APK file size cap.
//! - 128 MiB per-mapping file size cap (real R8 mapping.txt files
//!   for large apps frequently hit 50-100 MiB. 128 caps the
//!   adversarial inflation case without false-rejecting legitimate
//!   large mappings).
//! - 16 MiB worker thread stack.
//! - I/O failures on the manifest write OR a slot that claims
//!   pairs but the blobs are missing DO panic — they invalidate
//!   the data.
//!
//! # Resumability
//!
//! Per the F-Droid sweep's pattern, the harness reads any existing
//! `swept-manifest-paired.tsv` at start and skips slots already
//! processed at ANY droidsaw_sha. Set
//! `DROIDSAW_R8_PAIRED_RESWEEP=1` for per-commit re-sweep.

use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod common;

use common::r8_canonical_marker::{
    descriptor_to_mapping_key, parse_block_outlined_marker,
};
use common::r8_mapping_outline::OutlineSet;

const MAX_APK_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DEX_BYTES: usize = 64 * 1024 * 1024;
// 128 MiB — real R8 mapping.txt files for large apps frequently
// hit 50-100 MiB. 128 caps the adversarial inflation case
// without false-rejecting legitimate large mappings.
const MAX_MAPPING_BYTES: u64 = 128 * 1024 * 1024;
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;
const MANIFEST_FILENAME: &str = "swept-manifest-paired.tsv";
const PAIRS_INDEX_RELATIVE_PATH: &str = "index/manifest-pairs.tsv";

fn corpus_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("DROIDSAW_R8_PAIRED_ROOT")?;
    let p = PathBuf::from(raw);
    if !p.is_dir() {
        return None;
    }
    Some(p)
}

fn droidsaw_sha() -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}

fn timestamp_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_iso8601_utc(secs)
}

fn format_iso8601_utc(epoch_secs: u64) -> String {
    let secs = epoch_secs % 60;
    let mins = (epoch_secs / 60) % 60;
    let hours = (epoch_secs / 3600) % 24;
    let mut days = epoch_secs / 86_400;
    let mut year: u64 = 1970;
    loop {
        let yd = if is_leap(year) { 366 } else { 365 };
        if days < yd {
            break;
        }
        days -= yd;
        year = year.saturating_add(1);
    }
    let month_lens: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month: u64 = 1;
    let mut day_of_month = days.saturating_add(1);
    for &ml in &month_lens {
        if day_of_month <= ml {
            break;
        }
        day_of_month -= ml;
        month = month.saturating_add(1);
    }
    format!("{year:04}-{month:02}-{day_of_month:02}T{hours:02}:{mins:02}:{secs:02}Z")
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone)]
struct PairRow {
    slot: String,
    apk_sha256: String,
    mapping_sha256: String,
    r8_version: String,
}

/// Read the pairs index TSV. Tolerates comment lines + a header
/// row. Expected columns (tab-separated): slot, apk_sha256,
/// mapping_sha256, r8_version. Extra columns to the right are
/// tolerated.
fn read_pairs_index(path: &Path) -> Vec<PairRow> {
    let f = File::open(path)
        .unwrap_or_else(|e| panic!("open pairs index {}: {e}", path.display()));
    let mut out = Vec::new();
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if line.starts_with('#') || line.is_empty() || line.starts_with("slot\t") {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 4 {
            continue;
        }
        let slot = cols[0].trim();
        if slot.is_empty() {
            continue;
        }
        // Slot must be alphanumeric or hyphen — defensive against
        // path traversal via `..` or `/` in slot identifiers.
        if !slot.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            panic!(
                "slot {slot:?} contains non-alphanumeric characters; refusing to construct blob path"
            );
        }
        out.push(PairRow {
            slot: slot.to_string(),
            apk_sha256: cols[1].trim().to_string(),
            mapping_sha256: cols[2].trim().to_string(),
            r8_version: cols[3].trim().to_string(),
        });
    }
    out
}

fn already_processed(manifest_path: &Path) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    if std::env::var("DROIDSAW_R8_PAIRED_RESWEEP").ok().as_deref() == Some("1") {
        return seen;
    }
    let Ok(f) = File::open(manifest_path) else {
        return seen;
    };
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if line.starts_with('#') || line.is_empty() || line.starts_with("slot\t") {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.is_empty() {
            continue;
        }
        seen.insert(cols[0].to_string());
    }
    seen
}

fn slot_blob_dir(root: &Path, slot: &str) -> PathBuf {
    root.join("blobs").join(slot)
}

fn read_capped(path: &Path, cap: u64) -> Vec<u8> {
    let meta = std::fs::symlink_metadata(path)
        .unwrap_or_else(|e| panic!("stat {}: {e}", path.display()));
    if meta.file_type().is_symlink() {
        panic!("{} is a symlink; refusing to follow", path.display());
    }
    if meta.len() > cap {
        panic!(
            "{} is {} bytes (> {} cap); refusing to read",
            path.display(),
            meta.len(),
            cap,
        );
    }
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn extract_classes_dexes_from_apk(apk_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Cursor;
    let cur = Cursor::new(apk_bytes);
    let mut zip = match zip::ZipArchive::new(cur) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("apk zip parse failed: {e}; treating as zero-DEX APK");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .filter(|n| n.starts_with("classes") && n.ends_with(".dex"))
        .collect();
    for name in names {
        let mut entry = match zip.by_name(&name) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.size() > MAX_DEX_BYTES as u64 {
            eprintln!(
                "skip {name}: declared size {} > {} cap",
                entry.size(),
                MAX_DEX_BYTES,
            );
            continue;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        let mut limited = (&mut entry).take(MAX_DEX_BYTES as u64);
        if let Err(e) = limited.read_to_end(&mut buf) {
            eprintln!("read {name} from apk: {e}; skipping");
            continue;
        }
        out.push((name, buf));
    }
    out
}

#[test]
fn r8_paired_corpus_sweep() {
    let handle = std::thread::Builder::new()
        .name("r8_paired_corpus_sweep_worker".into())
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
            "SKIP: DROIDSAW_R8_PAIRED_ROOT unset or does not point at a directory. \
             To sweep the mapping-paired corpus, set DROIDSAW_R8_PAIRED_ROOT to \
             the corpus root containing `index/manifest-pairs.tsv` + \
             `blobs/<slot>/{{app.apk, mapping.txt}}`."
        );
        return;
    };

    let index_path = root.join(PAIRS_INDEX_RELATIVE_PATH);
    if !index_path.is_file() {
        eprintln!("SKIP: no pairs index at {}", index_path.display());
        return;
    }

    let manifest_path = root.join(MANIFEST_FILENAME);
    let mut rows = read_pairs_index(&index_path);
    rows.sort_by(|a, b| a.slot.cmp(&b.slot));

    let ds_sha = droidsaw_sha();
    let already = already_processed(&manifest_path);
    let total_in_scope = rows.len();
    let mut already_skip_count = 0usize;
    rows.retain(|r| {
        if already.contains(&r.slot) {
            already_skip_count += 1;
            false
        } else {
            true
        }
    });
    if already_skip_count > 0 {
        eprintln!(
            "RESUME: skipping {already_skip_count} slots already in manifest. \
             Set DROIDSAW_R8_PAIRED_RESWEEP=1 for per-commit re-sweep."
        );
    }

    write_manifest_header_if_new(&manifest_path);

    let mut pairs_swept = 0usize;
    let mut total_tp = 0usize;
    let mut total_fp = 0usize;
    let mut total_fn = 0usize;

    for row in &rows {
        let slot = &row.slot;
        let blob_dir = slot_blob_dir(&root, slot);
        let apk_path = blob_dir.join("app.apk");
        let mapping_path = blob_dir.join("mapping.txt");
        if !apk_path.is_file() || !mapping_path.is_file() {
            panic!(
                "slot {slot}: missing apk={} mapping={}",
                apk_path.display(),
                mapping_path.display(),
            );
        }

        let apk_bytes = read_capped(&apk_path, MAX_APK_BYTES);
        let measured_apk_sha = sha256_hex(&apk_bytes);
        if measured_apk_sha != row.apk_sha256 {
            panic!(
                "slot {slot}: apk sha256 mismatch (manifest claims {} but measured {})",
                row.apk_sha256, measured_apk_sha,
            );
        }

        let mapping_bytes = read_capped(&mapping_path, MAX_MAPPING_BYTES);
        let measured_mapping_sha = sha256_hex(&mapping_bytes);
        if measured_mapping_sha != row.mapping_sha256 {
            panic!(
                "slot {slot}: mapping sha256 mismatch (manifest claims {} but measured {})",
                row.mapping_sha256, measured_mapping_sha,
            );
        }
        let mapping_text = String::from_utf8_lossy(&mapping_bytes).into_owned();
        let outlines = OutlineSet::parse(&mapping_text);

        let dexes = extract_classes_dexes_from_apk(&apk_bytes);
        let dex_count = dexes.len();
        let mut total_classes = 0usize;
        let mut marker_count = 0usize;
        let mut fired_tuples: BTreeSet<(String, String)> = BTreeSet::new();

        for (dex_name, dex_bytes) in &dexes {
            let dex = match droidsaw_dex::parser::DexFile::parse(dex_bytes, None) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("slot {slot} dex {dex_name}: parse fail {e:?}; skipping");
                    continue;
                }
            };
            let census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);
            for class_def in &dex.class_defs {
                if class_def.class_data_off == 0 {
                    continue;
                }
                total_classes = total_classes.saturating_add(1);
                let out = droidsaw_dex::classes::decompile_class_with_census(
                    &dex, dex_bytes, class_def, &census,
                );
                for line in out.lines() {
                    let Some(marker) = parse_block_outlined_marker(line) else {
                        continue;
                    };
                    let key_class = descriptor_to_mapping_key(marker.helper_class);
                    if key_class.is_empty() {
                        continue;
                    }
                    marker_count = marker_count.saturating_add(1);
                    fired_tuples.insert((key_class, marker.helper_method.to_string()));
                }
            }
        }

        // TP/FP/FN computation. TP = fired tuples that are in the
        // outline set. FP = fired tuples NOT in the outline set
        // (recogniser false positive). FN = outline-set tuples that
        // did NOT fire (recogniser missed a real outline).
        let mut tp_count = 0usize;
        let mut fp_count = 0usize;
        for (c, m) in &fired_tuples {
            if outlines.is_outlined(c, m) {
                tp_count = tp_count.saturating_add(1);
            } else {
                fp_count = fp_count.saturating_add(1);
            }
        }
        let outlined_in_mapping = outlines.outlined_count();
        // FN = outlined - matched. Matched is computed via the
        // per_kind_match_report's matched column summed over all
        // kinds (also equal to TP since OutlineSet stores ONE kind
        // per outlined tuple).
        let kind_report = outlines.per_kind_match_report(&fired_tuples);
        let fn_count = outlined_in_mapping.saturating_sub(tp_count);

        // Per-variant breakdown string.
        let per_variant: String = kind_report
            .iter()
            .map(|(kind, total, matched)| {
                format!("{}:total={},tp={}", kind.label(), total, matched)
            })
            .collect::<Vec<_>>()
            .join(";");

        append_manifest_row(
            &manifest_path,
            ManifestRowOut {
                slot,
                apk_sha256: &row.apk_sha256,
                mapping_sha256: &row.mapping_sha256,
                droidsaw_sha: &ds_sha,
                timestamp: &timestamp_utc_now(),
                r8_version: &row.r8_version,
                dex_count,
                class_count: total_classes,
                marker_count,
                outlined_in_mapping,
                tp_count,
                fp_count,
                fn_count,
                per_variant_breakdown: &per_variant,
            },
        );

        eprintln!(
            "slot {slot} ({}/{}): {} classes, {} markers, TP={} FP={} FN={} (outlined_in_mapping={})",
            pairs_swept + 1,
            rows.len(),
            total_classes,
            marker_count,
            tp_count,
            fp_count,
            fn_count,
            outlined_in_mapping,
        );

        total_tp = total_tp.saturating_add(tp_count);
        total_fp = total_fp.saturating_add(fp_count);
        total_fn = total_fn.saturating_add(fn_count);
        pairs_swept = pairs_swept.saturating_add(1);
    }

    eprintln!(
        "---\nAGGREGATE: {pairs_swept} pairs swept ({} skipped from resume; {} in scope), \
         total_TP={total_tp} total_FP={total_fp} total_FN={total_fn}",
        already_skip_count,
        total_in_scope,
    );
    if total_tp + total_fp > 0 {
        let prec = total_tp as f64 / (total_tp + total_fp) as f64;
        eprintln!("aggregate precision: {prec:.4} ({total_tp} TP / {} fired)", total_tp + total_fp);
    }
    if total_tp + total_fn > 0 {
        let rec = total_tp as f64 / (total_tp + total_fn) as f64;
        eprintln!("aggregate recall:    {rec:.4} ({total_tp} TP / {} outlined)", total_tp + total_fn);
    }
}

struct ManifestRowOut<'a> {
    slot: &'a str,
    apk_sha256: &'a str,
    mapping_sha256: &'a str,
    droidsaw_sha: &'a str,
    timestamp: &'a str,
    r8_version: &'a str,
    dex_count: usize,
    class_count: usize,
    marker_count: usize,
    outlined_in_mapping: usize,
    tp_count: usize,
    fp_count: usize,
    fn_count: usize,
    per_variant_breakdown: &'a str,
}

fn write_manifest_header_if_new(manifest_path: &Path) {
    if manifest_path.exists() {
        return;
    }
    let header = "# Each row is one (APK, mapping.txt) pair sweep at one droidsaw commit.\n\
        # Columns are tab-separated.\n\
        # per_variant_breakdown is `;`-separated <kind>:total=N,tp=M pairs in SyntheticKind\n\
        # report order (Outline, CovariantOutline, ApiModelOutline, NonStartupInStartupOutline,\n\
        # BottomUpOutline, ObjectCloneOutline, LegacyGeneratedOutlineSupport, EnumUnboxing,\n\
        # OutlineKindUnknown, Unknown). `total` is the count of outline annotations of that\n\
        # kind in the mapping; `tp` is the count of fired markers that matched.\n\
        slot\tapk_sha256\tmapping_sha256\tdroidsaw_sha\ttimestamp_utc\tr8_version\tdex_count\tclass_count\tmarker_count\toutlined_in_mapping\ttp_count\tfp_count\tfn_count\tper_variant_breakdown\n";
    if let Err(e) = std::fs::write(manifest_path, header) {
        panic!("write manifest header to {}: {e}", manifest_path.display());
    }
}

fn append_manifest_row(manifest_path: &Path, row: ManifestRowOut<'_>) {
    let mut f = match OpenOptions::new().append(true).open(manifest_path) {
        Ok(f) => f,
        Err(e) => panic!(
            "open manifest {} for append: {e}",
            manifest_path.display(),
        ),
    };
    if row.slot.contains('\t') || row.slot.contains('\n') {
        panic!("slot contains TSV-corrupting whitespace: {:?}", row.slot);
    }
    let line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        row.slot,
        row.apk_sha256,
        row.mapping_sha256,
        row.droidsaw_sha,
        row.timestamp,
        row.r8_version,
        row.dex_count,
        row.class_count,
        row.marker_count,
        row.outlined_in_mapping,
        row.tp_count,
        row.fp_count,
        row.fn_count,
        row.per_variant_breakdown,
    );
    if let Err(e) = f.write_all(line.as_bytes()) {
        panic!("write manifest row to {}: {e}", manifest_path.display());
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
    fn sha256_hex_known() {
        let h = sha256_hex(b"abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
    }

    #[test]
    fn read_pairs_index_tolerates_comments_and_header() {
        let tmp = std::env::temp_dir().join(format!(
            "test_pairs_{}.tsv",
            std::process::id()
        ));
        std::fs::write(
            &tmp,
            "# comment line\nslot\tapk_sha256\tmapping_sha256\tr8_version\n\
             slot01\taaa\tbbb\t9.1.31\n\
             slot02\tccc\tddd\t9.0.32\n",
        )
        .unwrap();
        let rows = read_pairs_index(&tmp);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].slot, "slot01");
        assert_eq!(rows[1].r8_version, "9.0.32");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    #[should_panic(expected = "non-alphanumeric")]
    fn read_pairs_index_rejects_path_traversal_in_slot() {
        let tmp = std::env::temp_dir().join(format!(
            "test_pairs_bad_{}.tsv",
            std::process::id()
        ));
        std::fs::write(&tmp, "..\\evil\taaa\tbbb\t9.1.31\n").unwrap();
        let _ = read_pairs_index(&tmp);
        let _ = std::fs::remove_file(&tmp);
    }
}
