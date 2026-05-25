//! Per-APK UNRECOGNIZED_REGION ratchet test.
//!
//! For each entry in `tests/baselines/unrecognized.toml`:
//!   1. SKIP cleanly if the APK file is absent (corpus is opt-in —
//!      APKs are dev-box-specific; CI / fresh checkouts don't have
//!      them).
//!   2. FAIL LOUD if the file's SHA-256 differs from the pinned baseline
//!      ("APK rotated; baseline stale — re-baseline explicitly"). This
//!      preserves the per-APK monotone invariant when files at the same
//!      path rotate.
//!   3. Extract every `classes*.dex` from the APK, parse via
//!      `droidsaw_dex::parser::parse_dex`, and call
//!      `droidsaw_dex::diag::collect_unrecognized_findings` per dex layer.
//!   4. Sum the per-dex counts into a single per-APK count.
//!   5. Assert `actual <= baseline_count`. Reductions are improvements
//!      (the baseline tightens manually via a follow-up commit — that's
//!      the "monotone-decreasing" discipline).
//!
//! Per Brief Directive 2: per-APK monotone, NOT global. Each APK is gated
//! against its own baseline.
//!
//! Stream: `dex-unrecognized-diag` (#43 / PR-3).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

#[derive(Debug, serde::Deserialize)]
struct Baselines {
    #[allow(dead_code)] // schema discriminator; toml::de validates
    version: u32,
    apk: Vec<ApkBaseline>,
}

#[derive(Debug, serde::Deserialize)]
struct ApkBaseline {
    path: String,
    sha256: String,
    baseline_count: u32,
    #[serde(default)]
    #[allow(dead_code)] // documentation-only, not asserted
    notes: Option<String>,
}

fn load_baselines() -> Baselines {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("baselines")
        .join("unrecognized.toml");
    let text =
        fs::read_to_string(&path).expect("read tests/baselines/unrecognized.toml");
    toml::from_str(&text).expect("parse unrecognized.toml")
}

fn sha256_of_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for sha256");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn extract_classes_dex_blobs(apk_path: &Path) -> Vec<Vec<u8>> {
    let f = fs::File::open(apk_path).expect("open apk");
    let mut zip = zip::ZipArchive::new(f).expect("open zip");
    let mut out = Vec::new();
    let count = zip.len();
    for i in 0..count {
        let mut entry = zip.by_index(i).expect("zip entry");
        let name = entry.name().to_string();
        // Match `classes.dex`, `classes2.dex`, …, `classes999.dex` —
        // top-level only (no subdirectory `classes.dex`-named entries).
        if !name.starts_with("classes") || !name.ends_with(".dex") || name.contains('/') {
            continue;
        }
        let mut blob = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut blob).expect("read dex entry");
        out.push(blob);
    }
    out
}

fn count_unrecognized_in_apk(apk_path: &Path) -> u32 {
    // Spawn the audit in a dedicated thread with a 16 MB stack —
    // adversarial / R8-rewritten inputs drive the
    // structurer deeper than the default 2 MB test-thread can hold.
    // Mirrors `tests/roundtrip_kotlinc.rs`'s per-fixture stack discipline.
    let apk_path = apk_path.to_path_buf();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let blobs = extract_classes_dex_blobs(&apk_path);
            let mut total: u32 = 0;
            for blob in &blobs {
                let dex = match droidsaw_dex::parser::DexFile::parse(blob, None) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                // `collect_unrecognized_findings` returns at most one
                // rolled-up Finding per DEX whose `detail` leads with
                // `"<N> unrecognized region..."`. Parse the leading
                // integer to recover the per-region count for ratchet
                // comparison.
                for f in droidsaw_dex::diag::collect_unrecognized_findings(&dex, blob) {
                    let n: u32 = f
                        .detail
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1);
                    total = total.saturating_add(n);
                }
            }
            total
        })
        .expect("spawn audit thread")
        .join()
        .expect("audit thread panicked (likely stack overflow past 16 MB cap)")
}

#[test]
fn unrecognized_region_per_apk_ratchet() {
    // Debug-mode short-circuit: the audit pipeline runs ~10× slower in
    // debug than release on real APKs (~5min debug vs ~30s release per
    // audit). Default `cargo test` runs in debug; CI
    // runs with `--release` for the ratchet sweep. The fixture-corpus
    // zero-sentinel test (`fixture_unrecognized_zero.rs`) catches
    // in-tree regressions on every `cargo test` invocation regardless
    // of mode, so debug-skip here doesn't gap the regression net.
    if cfg!(debug_assertions) {
        eprintln!(
            "unrecognized_ratchet: SKIPPED in debug mode \
             (run `cargo test --release --test unrecognized_ratchet` for the full sweep). \
             In-tree fixtures still gated by `fixture_unrecognized_zero.rs`."
        );
        return;
    }

    let baselines = load_baselines();
    let mut skipped: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut checked: u32 = 0;

    for entry in &baselines.apk {
        let path = PathBuf::from(&entry.path);
        if !path.exists() {
            skipped.push(format!(
                "  - {} (file absent — corpus is opt-in)",
                entry.path
            ));
            continue;
        }

        // Load-bearing SHA pin: a content rotation must surface as a loud
        // failure, not a silent re-baseline.
        let actual_sha = sha256_of_file(&path);
        if actual_sha != entry.sha256 {
            errors.push(format!(
                "  - {}: APK rotated; baseline stale — re-baseline explicitly. \
                 expected sha256={}; got sha256={}",
                entry.path, entry.sha256, actual_sha,
            ));
            continue;
        }

        let actual = count_unrecognized_in_apk(&path);
        if actual > entry.baseline_count {
            errors.push(format!(
                "  - {}: regression. expected ≤ {} UNRECOGNIZED_REGION findings; \
                 got {}. Either fix the regression or update baseline_count.",
                entry.path, entry.baseline_count, actual,
            ));
        }
        // actual < baseline_count is an improvement; the baseline is
        // tightened manually via a follow-up commit.
        checked = checked.saturating_add(1);
    }

    if !skipped.is_empty() {
        eprintln!(
            "unrecognized_ratchet: {} APK(s) skipped (corpus opt-in):\n{}",
            skipped.len(),
            skipped.join("\n"),
        );
    }
    eprintln!(
        "unrecognized_ratchet: {checked} APK(s) checked, {} skipped, {} error(s)",
        skipped.len(),
        errors.len(),
    );

    if !errors.is_empty() {
        panic!(
            "{} ratchet violation(s):\n{}",
            errors.len(),
            errors.join("\n"),
        );
    }
}
