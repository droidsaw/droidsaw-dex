//! Env-gated production-corpus smoke test for the R8 inversion pass.
//!
//! Reads `DROIDSAW_R8_PROD_CORPUS_PATH` (a directory) and walks every
//! `.dex` file under it, decompiling each class and counting how many
//! method bodies the BlockOutlined recogniser tags with the
//! `R8Origin(BlockOutlined, …)` marker. Reports the count per DEX +
//! the total.
//!
//! Production APK class extracts (real-world R8-processed classes) are the
//! empirical floor for tuning the recogniser. Drop production DEXes into
//! a local directory; set the env var; observe what fires.
//!
//! # Adversarial-input discipline
//!
//! `DROIDSAW_R8_PROD_CORPUS_PATH` is analyst-supplied — the path may
//! contain attacker-controlled DEX bytes (third-party APK extracts,
//! customer engagement artefacts, leaked corpora) and the directory
//! structure itself can be hostile (symlinks, hardlinks, traversal
//! shapes). The smoke test enforces the same defences the R8 oracle
//! ratchet (`r8_oracle_ratchet.rs`) does:
//!
//! - Symlink reject at directory walk + per-file `fs::symlink_metadata`
//!   refuses to follow a `corpus/loop -> /` traversal.
//! - 64 MiB per-DEX size cap before `fs::read`. A crafted 4 GiB DEX
//!   with absurd `class_defs_size` would otherwise OOM the test
//!   runner.
//! - Strict marker-frame parser (mirrors the ratchet's
//!   `parse_marker_body`): a marker counts only if its line matches
//!   the canonical 4-field shape `<variant>, helper=<L...;>-><name>,
//!   callers=<digits>, confidence=<digits>` exactly. A DEX containing
//!   a string constant or method name that looks like a marker no
//!   longer inflates the count.
//!
//! Skips cleanly when the env var is unset.

use std::path::{Path, PathBuf};

/// Per-DEX size cap (matches the ratchet's mapping.txt cap). Larger
/// inputs are crafted-resource-exhaustion candidates; refuse the
/// read entirely.
const MAX_DEX_BYTES: u64 = 64 * 1024 * 1024;

mod common;

use common::r8_canonical_marker::count_block_outlined_markers;

fn corpus_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("DROIDSAW_R8_PROD_CORPUS_PATH")?;
    let p = PathBuf::from(raw);
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

/// Walk `root` and return `.dex` paths. Rejects symlinks anywhere
/// in the walk — a hostile `corpus/loop -> /` would otherwise let
/// the walk read root recursively for `.dex` files.
fn collect_dex_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let entry = entry?;
            let p = entry.path();
            // Use `symlink_metadata` so we see the entry itself,
            // not the symlink target. If the entry is a symlink at
            // any kind, skip — refuse to traverse into or read it.
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

/// Read a DEX file with size + symlink defences. Returns `None`
/// (with a `WARN: ...` log to stderr) on any refusal: missing file,
/// symlink, file larger than `MAX_DEX_BYTES`, or read I/O error.
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

// Canonical-shape marker parsing lives at
// `tests/common/r8_canonical_marker.rs`. The shared implementation
// is exercised by this smoke test, by the mapping-paired ratchet, and
// by its own unit tests.

/// Belt-and-suspenders defence against deep-recursion shapes that
/// the `cargo test` default 2 MiB thread stack would overflow.
///
/// The original overflow at depth ~130 was caused by
/// `Stmt::If::else_body` cascades (R8's structurer emits long else-if chains
/// from cascaded-switch-on-string lowerings). `emit::emit_stmt_depth` has
/// since been converted to iterative else-chain emission, so that
/// specific recursion pattern is bounded by `O(1)` stack depth, not `O(N)`.
///
/// We keep the 16 MiB explicit-stack wrapper because production R8
/// output may surface OTHER deep recursion shapes that haven't been
/// converted yet — `Stmt::While` body, `Stmt::TryCatch`, deep
/// `Stmt::Seq` nesting. Removing this wrapper requires a corpus
/// pass establishing those shapes don't trip the 2 MiB budget. The
/// `MAX_STMT_DEPTH = 512` cap inside `emit_stmt_depth` is the
/// production-side defence; this is the test-side belt.
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

#[test]
fn r8_production_corpus_block_outlining_smoke() {
    let handle = std::thread::Builder::new()
        .name("r8_production_corpus_smoke_worker".into())
        .stack_size(SMOKE_TEST_STACK_BYTES)
        .spawn(smoke_main)
        .expect("spawn stack-sized worker thread");
    if let Err(e) = handle.join() {
        std::panic::resume_unwind(e);
    }
}

fn smoke_main() {
    let Some(dir) = corpus_dir() else {
        eprintln!(
            "SKIP: DROIDSAW_R8_PROD_CORPUS_PATH unset or does not point at a directory. \
             To exercise the BlockOutlined recogniser against production R8 output, \
             drop one or more `.dex` files into a directory (e.g. extracted from \
             a known-R8-released APK like Threads dex11) and re-run with \
             DROIDSAW_R8_PROD_CORPUS_PATH=/path/to/dir. Measurement-only — does \
             not gate CI."
        );
        return;
    };
    let dexes = match collect_dex_files(&dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("SKIP: failed to walk {}: {e}", dir.display());
            return;
        }
    };
    if dexes.is_empty() {
        eprintln!(
            "SKIP: no `.dex` files under {}. Add production DEX extracts and re-run.",
            dir.display(),
        );
        return;
    }

    let mut total_markers = 0usize;
    let mut total_classes = 0usize;
    let mut total_methods_attempted = 0usize;
    let mut dexes_with_markers = 0usize;
    let mut dexes_skipped = 0usize;
    for dex_path in &dexes {
        let Some(data) = read_dex_capped(dex_path) else {
            dexes_skipped = dexes_skipped.saturating_add(1);
            continue;
        };
        let dex = match droidsaw_dex::parser::DexFile::parse(&data, None) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("WARN: skipping {} — parse failed: {e:?}", dex_path.display());
                dexes_skipped = dexes_skipped.saturating_add(1);
                continue;
            }
        };
        // Build the trampoline census ONCE per DEX, before the
        // per-class loop. `decompile_class_with_census` reuses it
        // per class — turns O(C × (M + I)) per-DEX work into
        // O(M + I). On a 7000-class production DEX (Threads
        // classes11) this is the difference between minutes and
        // seconds; the per-class `decompile_class` entry point
        // would otherwise rebuild the same census every iteration.
        let r8_census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);
        let mut markers_in_this_dex = 0usize;
        for class_def in &dex.class_defs {
            total_classes = total_classes.saturating_add(1);
            // Skip classes with no class_data (no methods to recognise).
            if class_def.class_data_off == 0 {
                continue;
            }
            let out = droidsaw_dex::classes::decompile_class_with_census(
                &dex, &data, class_def, &r8_census,
            );
            let m = count_block_outlined_markers(&out);
            markers_in_this_dex = markers_in_this_dex.saturating_add(m);
            // Approximate method count via line scan (any line ending in
            // `{` after stripping trailing whitespace is a method open;
            // good enough for ratio reporting).
            total_methods_attempted = total_methods_attempted
                .saturating_add(out.lines().filter(|l| l.trim_end().ends_with('{')).count());
        }
        if markers_in_this_dex > 0 {
            dexes_with_markers = dexes_with_markers.saturating_add(1);
        }
        eprintln!(
            "DEX {}: {} BlockOutlined markers",
            dex_path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            markers_in_this_dex,
        );
        total_markers = total_markers.saturating_add(markers_in_this_dex);
    }

    eprintln!(
        "PRODUCTION CORPUS SUMMARY: {} DEXes scanned ({} skipped, {} with ≥1 marker), \
         {} classes, ~{} methods, {} BlockOutlined markers total",
        dexes.len(),
        dexes_skipped,
        dexes_with_markers,
        total_classes,
        total_methods_attempted,
        total_markers,
    );
    // No assertion — this is a measurement gauge. The output goes to
    // stderr for analyst inspection. CI green/red is decoupled from
    // coverage; the goal is observation, not regression-gating.
}

// Marker-parse unit tests live with the shared parser at
// `tests/common/r8_canonical_marker.rs`.
