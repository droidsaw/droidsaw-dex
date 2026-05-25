//! Env-gated mapping-paired ratchet harness.
//!
//! Reads `DROIDSAW_R8_MAPPED_B_PATH` — a directory containing
//! `classes*.dex` plus the corresponding `mapping.txt`. Walks the
//! DEX files, decompiles each class, extracts BlockOutlined markers
//! emitted by `r8_inversion::apply`, and cross-references each
//! marker against the outline-set parsed from the mapping.
//!
//! Skips cleanly when the env var is unset (CI-friendly). Reports
//! per-corpus precision: marker total, TPs (helpers the mapping
//! declares outlined), FPs (helpers the mapping does NOT declare
//! outlined). Measurement-only — does not gate CI green/red.
//!
//! # Trust model
//!
//! This harness measures recogniser precision under COOPERATIVE
//! input: the analyst supplies a (DEX, mapping.txt) pair from the
//! same R8 invocation. What the harness trusts:
//!
//! - The mapping.txt is the genuine R8-emitted artefact for the
//!   accompanying DEX files (not a hand-crafted file whose
//!   `com.android.tools.r8.outline` annotations name methods that
//!   are NOT actually outline-emitted by R8).
//! - The DEX files were produced by the SAME R8 invocation that
//!   produced the mapping (no version skew, no post-emit DEX
//!   editing).
//!
//! What the harness does NOT defend against:
//!
//! - Adversarial mapping-honesty. A crafted mapping that annotates
//!   developer-written methods with `com.android.tools.r8.outline`
//!   would inflate the recogniser's apparent precision (every
//!   structural marker on a falsely-annotated method counts as TP).
//!   Defending this requires an INDEPENDENT oracle for what R8
//!   would have outlined — out of scope for a mapping-paired
//!   harness by construction.
//! - Cryptographic binding between DEX and mapping. There is no
//!   signature gate; a malicious analyst can swap either side of
//!   the pair to make recogniser numbers look better OR worse.
//! - R8-version compatibility. The harness assumes the mapping's
//!   annotations match the R8 version that produced the DEX; an
//!   older R8 with a different annotation shape produces UNKNOWN-
//!   bucket attributions (see SyntheticKind::Unknown in
//!   `r8_mapping_outline.rs`), not honesty violations.
//!
//! Bottom line: this is a precision MEASUREMENT under cooperative
//! input — not a recogniser-honesty oracle under adversarial input.
//!
//! # Adversarial-input discipline (resource exhaustion only)
//!
//! The corpus path is analyst-supplied. Defenses against
//! denial-of-service via the path itself (NOT against trust-model
//! violations above):
//!
//! - Symlink reject per directory entry + per file open.
//! - DEX size cap (`MAX_DEX_BYTES`) before any read.
//! - Worker thread spawned with a non-default stack so deeply
//!   nested emit IR does not overflow the test-runner's default
//!   thread stack (`SMOKE_TEST_STACK_BYTES`).

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::r8_canonical_marker::{descriptor_to_mapping_key, parse_block_outlined_marker};
use common::r8_mapping_outline::OutlineSet;

const MAX_DEX_BYTES: u64 = 64 * 1024 * 1024;
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Pinned mapping-disagreement allowlist for the mapping-paired
/// corpus B (presence-test sample). Empty by construction — the
/// corpus's tracked version ships only 2 outline annotations across
/// 63k classes (presence-test sample size per the validation gauge
/// thresholds), and no mapping-disagreeing FPs have been observed
/// against it yet.
///
/// Format mirrors `r8_mapping_paired_a_ratchet::FP_ALLOWLIST`:
/// `(mapping_key_class, helper_method_or_wildcard, justification)`.
/// `*` in the method slot means any method on the class qualifies.
const FP_ALLOWLIST: &[(&str, &str, &str)] = &[];

/// True if the (class, method) FP is in [`FP_ALLOWLIST`].
fn fp_is_allowlisted(class: &str, method: &str) -> bool {
    FP_ALLOWLIST
        .iter()
        .any(|(c, m, _)| *c == class && (*m == "*" || *m == method))
}

fn corpus_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("DROIDSAW_R8_MAPPED_B_PATH")?;
    let p = PathBuf::from(raw);
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
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

#[test]
fn r8_mapping_paired_b_baseline() {
    let handle = std::thread::Builder::new()
        .name("r8_mapping_paired_b_worker".into())
        .stack_size(SMOKE_TEST_STACK_BYTES)
        .spawn(baseline_main)
        .expect("spawn stack-sized worker thread");
    if let Err(e) = handle.join() {
        std::panic::resume_unwind(e);
    }
}

fn baseline_main() {
    let Some(dir) = corpus_dir() else {
        eprintln!(
            "SKIP: DROIDSAW_R8_MAPPED_B_PATH unset or does not point at a directory. \
             To exercise the mapping-paired corpus B ratchet, prepare a directory with \
             R8 mapping pairs (classes*.dex + mapping.txt files), then set \
             DROIDSAW_R8_MAPPED_B_PATH=/path/to/dir."
        );
        return;
    };
    let mapping_path = dir.join("mapping.txt");
    if !mapping_path.is_file() {
        eprintln!("SKIP: no mapping.txt at {}", mapping_path.display());
        return;
    }
    let outlines = match OutlineSet::from_file(&mapping_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SKIP: failed to parse mapping.txt: {e}");
            return;
        }
    };

    let dexes = match collect_dex_files(&dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("SKIP: failed to walk {}: {e}", dir.display());
            return;
        }
    };
    if dexes.is_empty() {
        eprintln!("SKIP: no `.dex` files under {}", dir.display());
        return;
    }

    let mut total_classes = 0usize;
    let mut total_methods = 0usize;
    let mut total_markers = 0usize;
    let mut tp_count = 0usize;
    let mut fp_marker_lines: Vec<String> = Vec::new();
    let mut dexes_skipped = 0usize;
    // Set of mapping-key class names (descriptor `Laf/m;` →
    // mapping key `af.m`) the harness iterated over and called
    // decompile_class_with_census on. The recogniser had its
    // chance to fire on every class in this set; absence here
    // means the class isn't in any of the corpus's DEX files
    // (tree-shaken, in an APK variant not extracted, or named in
    // mapping but not realised in bytecode).
    let mut classes_visited: BTreeSet<String> = BTreeSet::new();
    let mut classes_failed_decompile = 0usize;
    // Set of (mapping-key class, helper method) tuples for which a
    // BlockOutlined marker fired. Cross-referenced against the
    // outline set's per-kind attribution to build the
    // per-SyntheticKind precision breakdown.
    let mut fired_markers: BTreeSet<(String, String)> = BTreeSet::new();
    // FPs not covered by [`FP_ALLOWLIST`]. Set rather than Vec so a
    // single FP showing up at multiple call sites is counted once.
    // Non-empty at the end of the walk fails the ratchet.
    let mut unallowlisted_fps: BTreeSet<(String, String)> = BTreeSet::new();

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
        let census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);
        for class_def in &dex.class_defs {
            total_classes = total_classes.saturating_add(1);
            if class_def.class_data_off == 0 {
                continue;
            }
            let class_desc = dex
                .type_descriptors
                .get(class_def.class_idx.0 as usize)
                .map(String::as_str)
                .unwrap_or("?");
            classes_visited.insert(descriptor_to_mapping_key(class_desc));
            let out = droidsaw_dex::classes::decompile_class_with_census(
                &dex, &data, class_def, &census,
            );
            if out.contains("// Failed to decompile:") {
                classes_failed_decompile = classes_failed_decompile.saturating_add(1);
            }
            total_methods = total_methods.saturating_add(
                out.lines().filter(|l| l.trim_end().ends_with('{')).count(),
            );
            for line in out.lines() {
                let Some(marker) = parse_block_outlined_marker(line) else {
                    continue;
                };
                let key_class = descriptor_to_mapping_key(marker.helper_class);
                total_markers = total_markers.saturating_add(1);
                if outlines.is_outlined(&key_class, marker.helper_method) {
                    tp_count = tp_count.saturating_add(1);
                    fired_markers.insert((key_class.clone(), marker.helper_method.to_string()));
                } else {
                    let allowlisted = fp_is_allowlisted(&key_class, marker.helper_method);
                    let tag = if allowlisted { "FP (allowlisted)" } else { "FP" };
                    fp_marker_lines.push(format!(
                        "{tag}: {class_desc} marker {}->{} (key {key_class}.{}) not outline-annotated",
                        marker.helper_class, marker.helper_method, marker.helper_method,
                    ));
                    if !allowlisted {
                        unallowlisted_fps.insert((key_class, marker.helper_method.to_string()));
                    }
                }
            }
        }
    }

    // Class-visit coverage: of the outline-annotated classes in
    // the mapping, how many had their containing class visited
    // by the harness? A class missing here means the corpus's DEX
    // files don't contain that class. Without this check, "0
    // markers" hides whether the recogniser got a chance to fire.
    let mut outline_classes_visited = 0usize;
    let mut outline_classes_missing: Vec<(String, String)> = Vec::new();
    for (class, method) in outlines.outlined_methods() {
        if classes_visited.contains(class) {
            outline_classes_visited = outline_classes_visited.saturating_add(1);
        } else {
            outline_classes_missing.push((class.to_string(), method.to_string()));
        }
    }

    let fp_count = fp_marker_lines.len();
    let precision = if total_markers > 0 {
        100.0 * tp_count as f64 / total_markers as f64
    } else {
        0.0
    };

    eprintln!(
        "MAPPING-PAIRED CORPUS B BASELINE: {} DEXes ({} skipped), \
         {} classes ({} decompile-failed), ~{} methods, \
         {} markers fired ({} TP / {} FP = {:.1}% precision)",
        dexes.len(),
        dexes_skipped,
        total_classes,
        classes_failed_decompile,
        total_methods,
        total_markers,
        tp_count,
        fp_count,
        precision,
    );
    eprintln!(
        "DECOMPILE-COVERAGE: {} of {} outline-annotated CLASSES visited by harness ({:.1}%)",
        outline_classes_visited,
        outlines.outlined_count(),
        if outlines.outlined_count() > 0 {
            100.0 * outline_classes_visited as f64 / outlines.outlined_count() as f64
        } else {
            0.0
        },
    );

    // Per-SyntheticKind breakdown. Replaces the bulk-aggregate
    // "42/45 = 93%" reading with one row per outline-emitting kind
    // so kind-specific recogniser misses surface explicitly. Rows
    // emitted in stable R8-source declaration order.
    eprintln!("PER-KIND BREAKDOWN (annotated / matched-by-marker / recall):");
    let report = outlines.per_kind_match_report(&fired_markers);
    for (kind, annotated, matched) in &report {
        let recall = if *annotated > 0 {
            100.0 * (*matched as f64) / (*annotated as f64)
        } else {
            0.0
        };
        eprintln!(
            "  {:<32} {:>4} / {:>4}  ({:.1}%)",
            kind.label(),
            annotated,
            matched,
            recall,
        );
    }

    eprintln!(
        "MAPPING SET SIZES: {} outline-body annotations, {} outline-callsite annotations, \
         {} duplicate outline tuples, {} callsite helper-conflicts, {} proximity-dropped",
        outlines.outlined_count(),
        outlines.outline_callsite_count(),
        outlines.duplicate_outline_methods().len(),
        outlines.callsite_helper_conflicts().len(),
        outlines.outline_proximity_dropped(),
    );
    if outlines.cap_tripped() {
        eprintln!("WARN: outline-set body parser hit MAX_OUTLINE_METHODS cap; partial parse");
    }
    if outlines.callsite_cap_tripped() {
        eprintln!("WARN: outline-set callsite parser hit MAX_OUTLINE_CALLSITES cap; partial parse");
    }

    // Per-FP detail with allowlist annotation (does not affect the
    // mapping-mismatch gate below — purely diagnostic).
    for fp in fp_marker_lines.iter().take(10) {
        eprintln!("  {fp}");
    }
    if fp_count > 10 {
        eprintln!("  ... and {} more FPs (showing first 10)", fp_count - 10);
    }
    for (c, m) in outline_classes_missing.iter().take(10) {
        eprintln!("  outline-class not in any visited DEX: {c}.{m}");
    }
    if outline_classes_missing.len() > 10 {
        eprintln!(
            "  ... and {} more outline-annotated classes not in any visited DEX (showing first 10)",
            outline_classes_missing.len() - 10,
        );
    }

    // Mapping-mismatch gate. Any FP NOT in the (currently empty)
    // FP_ALLOWLIST fails the ratchet hard. See corpus A's allowlist
    // for the established format + justification expectation.
    if !unallowlisted_fps.is_empty() {
        for (c, m) in &unallowlisted_fps {
            eprintln!("UNALLOWLISTED FP: {c}.{m}");
        }
        panic!(
            "{} mapping-disagreement FP(s) not in FP_ALLOWLIST. Either fix the \
             recogniser to decline on these, or add an entry to FP_ALLOWLIST with \
             a structural justification.",
            unallowlisted_fps.len(),
        );
    }
}
