//! Env-gated byte-identity sweep across the F-Droid APK-blob corpus.
//!
//! Sibling harness to `r8_fdroid_apk_sweep`; same shape (manifest TSV
//! with first-pass dedupe + shard support), different per-DEX work. For every `classes*.dex` in
//! every APK, runs:
//!   1. `DexFile::parse`
//!   2. emit under `preserve_data_section_layout + preserve_map_list_order
//!      + preserve_encoded_value_width` (the "default-emit byte-identity"
//!        mode — output is valid loadable DEX)
//!   3. emit under the above + `preserve_input_checksums = true` (the
//!      "audit-mode source-faithful" mode — output may be non-canonical
//!      DEX if input had wrong checksums)
//!   4. byte-compare output to input under both modes.
//!
//! Reads `$DROIDSAW_BYTEID_FDROID_ROOT` (the F-Droid mirror root,
//! containing `index/manifest-latest.tsv` + `blobs/<aa>/<sha256>.apk`).
//! Appends one row per DEX to `$ROOT/swept-manifest-byteid.tsv`.
//!
//! # Shard mode
//!
//! For parallel runs across N workers, set
//! `DROIDSAW_BYTEID_FDROID_SHARD=W/N` (0-indexed worker; e.g.
//! `0/16`, `15/16`). Each worker processes APKs where
//! `apk_sha256 % N == W`. Default unset = 0/1 (single process).
//!
//! # Sample mode
//!
//! For sanity-test before full-corpus runs, set
//! `DROIDSAW_BYTEID_FDROID_SAMPLE_N=<count>` to cap the number of
//! APKs processed. Picks the FIRST `count` rows after sort-by-sha256
//! (stable across runs).
//!
//! # Re-sweep
//!
//! Default behavior: skip APKs that already have a row in the manifest
//! at this droidsaw_sha. Set `DROIDSAW_BYTEID_FDROID_RESWEEP=1` to
//! re-process every APK regardless of prior rows.
//!
//! # Out-of-process discipline
//!
//! Per the F-Droid sweep playbook
//! (`feedback-fdroid-corpus-sweep-playbook`): pre-build the test
//! binary BEFORE spawning the worker swarm to avoid 16-way `target/`
//! file-lock contention. ETA × 2.5 from the warm sample-run rate —
//! the long tail of multi-DEX APKs dominates real runtime.

use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use droidsaw_dex::emit_dex::{emit_dex_collect, EmitConfig};
use droidsaw_dex::parser::DexFile;

const MAX_APK_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DEX_BYTES: usize = 64 * 1024 * 1024;
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;
const MANIFEST_FILENAME: &str = "swept-manifest-byteid.tsv";
const INDEX_RELATIVE_PATH: &str = "index/manifest-latest.tsv";

const ENV_ROOT: &str = "DROIDSAW_BYTEID_FDROID_ROOT";
const ENV_SHARD: &str = "DROIDSAW_BYTEID_FDROID_SHARD";
const ENV_SAMPLE_N: &str = "DROIDSAW_BYTEID_FDROID_SAMPLE_N";
const ENV_RESWEEP: &str = "DROIDSAW_BYTEID_FDROID_RESWEEP";

struct ManifestRow {
    package: String,
    sha256: String,
}

fn corpus_dir() -> Option<PathBuf> {
    let raw = std::env::var_os(ENV_ROOT)?;
    let p = PathBuf::from(raw);
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

fn sample_n() -> Option<usize> {
    std::env::var(ENV_SAMPLE_N)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
}

/// Returns `(worker, n_workers)` from "W/N" form. Defaults to `(0, 1)`
/// when unset. Malformed values panic loudly — better than silently
/// running as 0/1 and producing duplicate rows.
fn shard() -> (u64, u64) {
    let Ok(raw) = std::env::var(ENV_SHARD) else {
        return (0, 1);
    };
    let parts: Vec<&str> = raw.split('/').collect();
    assert_eq!(
        parts.len(),
        2,
        "{ENV_SHARD}={raw:?} — expected 'W/N' form (e.g. 0/16)"
    );
    let w: u64 = parts[0].parse().unwrap_or_else(|e| {
        panic!("{ENV_SHARD}: worker index parse failed: {e}")
    });
    let n: u64 = parts[1].parse().unwrap_or_else(|e| {
        panic!("{ENV_SHARD}: shard count parse failed: {e}")
    });
    assert!(n > 0, "{ENV_SHARD}: shard count must be > 0");
    assert!(
        w < n,
        "{ENV_SHARD}: worker index {w} >= shard count {n}"
    );
    (w, n)
}

fn resweep_enabled() -> bool {
    std::env::var(ENV_RESWEEP).ok().as_deref() == Some("1")
}

fn droidsaw_sha() -> String {
    let out = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

fn timestamp_utc_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_iso8601_utc(now)
}

fn format_iso8601_utc(epoch_secs: u64) -> String {
    let secs = epoch_secs % 60;
    let mins = (epoch_secs / 60) % 60;
    let hours = (epoch_secs / 3600) % 24;
    let mut days = epoch_secs / 86_400;
    let mut year = 1970u64;
    loop {
        let dpy = if is_leap(year) { 366 } else { 365 };
        if days < dpy {
            break;
        }
        days -= dpy;
        year += 1;
    }
    let dim: [u64; 12] = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut month = 0usize;
    while month < 12 && days >= dim[month] {
        days -= dim[month];
        month += 1;
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month + 1,
        days + 1,
        hours,
        mins,
        secs,
    )
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn read_fdroid_manifest(path: &Path) -> Vec<ManifestRow> {
    let f = File::open(path).unwrap_or_else(|e| {
        panic!("could not open F-Droid manifest at {}: {e}", path.display())
    });
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let line = line.unwrap_or_else(|e| {
            panic!("read failed on line {line_no} of {}: {e}", path.display())
        });
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
    let meta = std::fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() {
        return None;
    }
    if meta.len() > MAX_APK_BYTES {
        return None;
    }
    std::fs::read(path).ok()
}

fn extract_dexes_from_apk(apk_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let cursor = std::io::Cursor::new(apk_bytes);
    let mut zip = match zip::ZipArchive::new(cursor) {
        Ok(z) => z,
        Err(_) => return Vec::new(),
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
            Err(_) => continue,
        };
        let size = entry.size();
        if size > MAX_DEX_BYTES as u64 {
            continue;
        }
        let mut buf = Vec::with_capacity(size.min(MAX_DEX_BYTES as u64) as usize);
        if entry.read_to_end(&mut buf).is_err() {
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

/// Dedupe key per playbook: `(droidsaw_sha, apk_sha256)`. First-pass
/// coverage stays stable across droidsaw-sha changes; same APK at a
/// later commit is a separate row.
fn already_processed(manifest_path: &Path) -> BTreeSet<(String, String)> {
    let mut seen = BTreeSet::new();
    let Ok(f) = File::open(manifest_path) else {
        return seen;
    };
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if line.starts_with('#') || line.starts_with("package\t") || line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 4 {
            continue;
        }
        // Columns: package, apk_sha256, droidsaw_sha, timestamp, …
        seen.insert((cols[2].to_string(), cols[1].to_string()));
    }
    seen
}

/// Shard predicate: `apk_sha256 % n == w`. Uses the first 16 hex chars
/// as a u64 (more than enough entropy; even distribution since sha256
/// is a CSPRNG).
fn in_shard(sha256: &str, worker: u64, n_workers: u64) -> bool {
    if n_workers <= 1 {
        return true;
    }
    let prefix = &sha256[..16.min(sha256.len())];
    let v = u64::from_str_radix(prefix, 16).unwrap_or(0);
    v % n_workers == worker
}

fn write_manifest_header_if_new(manifest_path: &Path) {
    if manifest_path.exists() {
        return;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(manifest_path)
        .unwrap_or_else(|e| {
            panic!("could not create manifest at {}: {e}", manifest_path.display())
        });
    let header = "\
package\tapk_sha256\tdroidsaw_sha\ttimestamp\tdex_name\tdex_sha256\tdex_bytes\t\
parse_ok\temit_default_ok\tbyte_id_default\tdiff_bytes_default\t\
emit_audit_ok\tbyte_id_audit\tdiff_bytes_audit\tnotes\n";
    let _ = f.write_all(header.as_bytes());
}

struct DexRowOut<'a> {
    package: &'a str,
    apk_sha256: &'a str,
    droidsaw_sha: &'a str,
    timestamp: &'a str,
    dex_name: &'a str,
    dex_sha256: &'a str,
    dex_bytes: usize,
    parse_ok: bool,
    emit_default_ok: bool,
    byte_id_default: bool,
    diff_bytes_default: u64,
    emit_audit_ok: bool,
    byte_id_audit: bool,
    diff_bytes_audit: u64,
    notes: &'a str,
}

fn append_dex_row(manifest_path: &Path, row: DexRowOut<'_>) {
    let mut f = match OpenOptions::new()
        .append(true)
        .open(manifest_path)
    {
        Ok(f) => f,
        Err(_) => return,
    };
    let line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        row.package,
        row.apk_sha256,
        row.droidsaw_sha,
        row.timestamp,
        row.dex_name,
        row.dex_sha256,
        row.dex_bytes,
        row.parse_ok as u8,
        row.emit_default_ok as u8,
        row.byte_id_default as u8,
        row.diff_bytes_default,
        row.emit_audit_ok as u8,
        row.byte_id_audit as u8,
        row.diff_bytes_audit,
        row.notes.replace(['\t', '\n'], " "),
    );
    let _ = f.write_all(line.as_bytes());
}

fn count_diff_bytes(a: &[u8], b: &[u8]) -> u64 {
    let common = a.len().min(b.len());
    let diff_in_common = (0..common).filter(|&i| a[i] != b[i]).count() as u64;
    let size_diff = a.len().abs_diff(b.len()) as u64;
    diff_in_common.saturating_add(size_diff)
}

#[test]
fn corpus_byte_identity_fdroid_sweep() {
    let handle = std::thread::Builder::new()
        .name("corpus_byte_identity_fdroid_sweep_worker".into())
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
            "SKIP: {ENV_ROOT} unset or does not point at a directory. \
             To sweep the F-Droid corpus, set {ENV_ROOT} to the \
             mirror root containing `index/manifest-latest.tsv` and \
             `blobs/<aa>/<sha256>.apk`. Sample mode via \
             {ENV_SAMPLE_N}=<N> caps APKs. Shard via {ENV_SHARD}=W/N."
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
    rows.sort_by(|a, b| a.sha256.cmp(&b.sha256));

    let (worker, n_workers) = shard();
    let total_unsharded = rows.len();
    rows.retain(|r| in_shard(&r.sha256, worker, n_workers));
    let after_shard = rows.len();

    if let Some(cap) = sample_n() {
        rows.truncate(cap);
        eprintln!(
            "SAMPLE MODE: capped to {} APKs ({ENV_SAMPLE_N})",
            rows.len()
        );
    }

    let ds_sha = droidsaw_sha();
    let resweep = resweep_enabled();
    let already = if resweep {
        BTreeSet::new()
    } else {
        already_processed(&manifest_path)
    };
    let before_dedup = rows.len();
    if !resweep {
        rows.retain(|r| !already.contains(&(ds_sha.clone(), r.sha256.clone())));
    }
    let after_dedup = rows.len();

    eprintln!(
        "byte-identity sweep: total={total_unsharded} shard={worker}/{n_workers} \
         after_shard={after_shard} after_dedup={after_dedup} \
         skipped_already={} droidsaw_sha={ds_sha} resweep={resweep}",
        before_dedup - after_dedup,
    );

    write_manifest_header_if_new(&manifest_path);

    let cfg_default = EmitConfig {
        preserve_data_section_layout: true,
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        ..Default::default()
    };
    let cfg_audit = EmitConfig {
        preserve_data_section_layout: true,
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        preserve_input_checksums: true,
        ..Default::default()
    };

    let mut apks_swept = 0usize;
    let mut dexes_total = 0usize;
    let mut dexes_byte_id_default = 0usize;
    let mut dexes_byte_id_audit = 0usize;
    let mut dexes_parse_fail = 0usize;
    let mut dexes_emit_fail = 0usize;

    for row in &rows {
        let blob_path = apk_blob_path(&root, &row.sha256);
        let Some(apk_bytes) = read_apk_capped(&blob_path) else {
            continue;
        };
        let dexes = extract_dexes_from_apk(&apk_bytes);
        if dexes.is_empty() {
            continue;
        }
        apks_swept = apks_swept.saturating_add(1);
        let ts = timestamp_utc_now();
        for (dex_name, data) in &dexes {
            dexes_total = dexes_total.saturating_add(1);
            let dex_sha = sha256_hex(data);
            let (parse_ok, dex_opt) = match DexFile::parse(data, None) {
                Ok(d) => (true, Some(d)),
                Err(_) => {
                    dexes_parse_fail = dexes_parse_fail.saturating_add(1);
                    (false, None)
                }
            };
            let (mut emit_default_ok, mut byte_id_default, mut diff_default) =
                (false, false, 0u64);
            let (mut emit_audit_ok, mut byte_id_audit, mut diff_audit) =
                (false, false, 0u64);
            let mut notes = String::new();
            if let Some(dex) = dex_opt {
                match emit_dex_collect(&dex, &cfg_default) {
                    Ok(out) => {
                        emit_default_ok = true;
                        byte_id_default = out.bytes == *data;
                        if !byte_id_default {
                            diff_default = count_diff_bytes(&out.bytes, data);
                        } else {
                            dexes_byte_id_default = dexes_byte_id_default.saturating_add(1);
                        }
                    }
                    Err(e) => {
                        dexes_emit_fail = dexes_emit_fail.saturating_add(1);
                        notes = format!("emit_default_err={e:?}");
                    }
                }
                match emit_dex_collect(&dex, &cfg_audit) {
                    Ok(out) => {
                        emit_audit_ok = true;
                        byte_id_audit = out.bytes == *data;
                        if !byte_id_audit {
                            diff_audit = count_diff_bytes(&out.bytes, data);
                        } else {
                            dexes_byte_id_audit = dexes_byte_id_audit.saturating_add(1);
                        }
                    }
                    Err(e) => {
                        if !notes.is_empty() {
                            notes.push_str("; ");
                        }
                        notes.push_str(&format!("emit_audit_err={e:?}"));
                    }
                }
            }
            append_dex_row(
                &manifest_path,
                DexRowOut {
                    package: &row.package,
                    apk_sha256: &row.sha256,
                    droidsaw_sha: &ds_sha,
                    timestamp: &ts,
                    dex_name,
                    dex_sha256: &dex_sha,
                    dex_bytes: data.len(),
                    parse_ok,
                    emit_default_ok,
                    byte_id_default,
                    diff_bytes_default: diff_default,
                    emit_audit_ok,
                    byte_id_audit,
                    diff_bytes_audit: diff_audit,
                    notes: &notes,
                },
            );
        }
        if apks_swept.is_multiple_of(50) {
            eprintln!(
                "  [{apks_swept} APKs / {dexes_total} DEXes] \
                 default_byte_id={dexes_byte_id_default} \
                 audit_byte_id={dexes_byte_id_audit} \
                 parse_fail={dexes_parse_fail} emit_fail={dexes_emit_fail}"
            );
        }
    }

    eprintln!(
        "\n=== byte-identity sweep complete ===\n\
         shard={worker}/{n_workers} apks_swept={apks_swept} dexes_total={dexes_total}\n\
         default_byte_id={dexes_byte_id_default} ({:.2}%)\n\
         audit_byte_id={dexes_byte_id_audit} ({:.2}%)\n\
         parse_fail={dexes_parse_fail} emit_fail={dexes_emit_fail}\n\
         manifest={}",
        100.0 * dexes_byte_id_default as f64 / dexes_total.max(1) as f64,
        100.0 * dexes_byte_id_audit as f64 / dexes_total.max(1) as f64,
        manifest_path.display(),
    );
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn shard_predicate_partitions_evenly() {
        // 4 shards across 1000 random-shape sha256 prefixes should
        // split roughly evenly. Just verify each predicate matches
        // ~25% (±5%).
        let prefixes: Vec<String> = (0u64..1000)
            .map(|i| format!("{:016x}deadbeef", i.wrapping_mul(0x9E3779B97F4A7C15u64)))
            .collect();
        for w in 0..4 {
            let count = prefixes.iter().filter(|p| in_shard(p, w, 4)).count();
            assert!(
                (200..=300).contains(&count),
                "shard {w}/4 saw {count} matches; expected ~250 ±50"
            );
        }
    }

    #[test]
    fn shard_predicate_default_passes_all() {
        assert!(in_shard("any", 0, 1));
    }

    #[test]
    fn iso8601_basic() {
        assert_eq!(format_iso8601_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_iso8601_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
