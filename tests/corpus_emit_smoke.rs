//! Opt-in corpus smoke test for `emit_dex`.
//!
//! Reads `DROIDSAW_DEX_CORPUS` env var; if unset the test is a no-op
//! so CI (which has no corpus on disk) stays green. Locally, run with:
//!
//! ```bash
//! DROIDSAW_DEX_CORPUS=/var/tmp/dex-corpus \
//!     cargo test --test corpus_emit_smoke -- --nocapture
//! ```
//!
//! The test walks `*.dex` files in the corpus dir and asserts
//! `parse(emit(parse(bytes)))` is content-equivalent to `parse(bytes)`.
//! Fails on the first divergence with the offending path in the
//! error message (`&'static str` error contexts would otherwise lose
//! the per-file locus).

use std::path::PathBuf;

use droidsaw_dex::emit_dex::{emit_dex_collect, map_type, CanonicalTransform, EmitConfig};
use droidsaw_dex::parser::{ContentEquiv, DexFile, MapEntry};

/// Map a DEX section type_code to a human-readable label for diagnostics.
fn type_code_label(tc: u16) -> &'static str {
    match tc {
        x if x == map_type::HEADER_ITEM => "header",
        x if x == map_type::STRING_ID_ITEM => "string_ids",
        x if x == map_type::TYPE_ID_ITEM => "type_ids",
        x if x == map_type::PROTO_ID_ITEM => "proto_ids",
        x if x == map_type::FIELD_ID_ITEM => "field_ids",
        x if x == map_type::METHOD_ID_ITEM => "method_ids",
        x if x == map_type::CLASS_DEF_ITEM => "class_defs",
        x if x == map_type::CALL_SITE_ID_ITEM => "call_site_ids",
        x if x == map_type::METHOD_HANDLE_ITEM => "method_handles",
        x if x == map_type::STRING_DATA_ITEM => "string_data",
        x if x == map_type::TYPE_LIST => "type_list",
        x if x == map_type::CODE_ITEM => "code_item",
        x if x == map_type::CLASS_DATA_ITEM => "class_data",
        x if x == map_type::ANNOTATION_ITEM => "annotation",
        x if x == map_type::ANNOTATION_SET_ITEM => "annotation_set",
        x if x == map_type::ANNOTATION_SET_REF_LIST => "annotation_set_ref_list",
        x if x == map_type::ANNOTATION_DIRECTORY_ITEM => "annotation_directory",
        x if x == map_type::ENCODED_ARRAY_ITEM => "encoded_array",
        x if x == map_type::DEBUG_INFO_ITEM => "debug_info",
        x if x == map_type::MAP_LIST => "map_list",
        _ => "unknown",
    }
}

/// Compute per-section byte ranges from the map_entries list. Sorts by
/// offset (independent of input's possibly-non-canonical order) and
/// derives each section's byte length from the next section's offset
/// (last section ends at file_size). Returns a Vec<(type_code, start, end)>.
fn section_ranges(file_size: usize, entries: &[MapEntry]) -> Vec<(u16, usize, usize)> {
    let mut sorted: Vec<MapEntry> = entries.to_vec();
    sorted.sort_by_key(|e| e.offset);
    let mut out = Vec::new();
    for (i, e) in sorted.iter().enumerate() {
        let start = e.offset as usize;
        let end = sorted
            .get(i + 1)
            .map(|n| n.offset as usize)
            .unwrap_or(file_size);
        out.push((e.type_code, start.min(file_size), end.min(file_size)));
    }
    out
}

fn corpus_root() -> Option<PathBuf> {
    std::env::var("DROIDSAW_DEX_CORPUS").ok().map(PathBuf::from)
}

fn walk_dex_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("dex") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn corpus_emit_roundtrip_smoke() {
    let Some(root) = corpus_root() else {
        eprintln!(
            "corpus_emit_roundtrip_smoke: DROIDSAW_DEX_CORPUS unset; skipping \
             (set to a directory of .dex files to run)"
        );
        return;
    };
    let dex_paths = walk_dex_files(&root);
    assert!(
        !dex_paths.is_empty(),
        "corpus root {root:?} has no .dex files — set DROIDSAW_DEX_CORPUS to a \
         dir with real DEX files (e.g., unpack APKs with `unzip -p apk.apk \
         classes*.dex > classes.dex`)"
    );

    let total = dex_paths.len();
    let mut emit_supported = 0usize;
    let mut emit_not_implemented = 0usize;
    let mut byte_identical = 0usize;
    let mut no_reported_transforms = 0usize;

    // Fail-fast discipline: on the FIRST unexpected failure, panic
    // with the offending path + error. The fix-continue loop requires
    // halting at the first finding so we can diagnose + patch without
    // burying it in a pile of downstream mismatches.

    for (i, path) in dex_paths.iter().enumerate() {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let Ok(dex1) = DexFile::parse(&bytes, None) else {
            // Parse failure isn't a round-trip bug; log + skip.
            eprintln!("  [{}/{total}] parse-fail: {}", i + 1, path.display());
            continue;
        };

        let out = match emit_dex_collect(&dex1, &EmitConfig::default()) {
            Ok(o) => {
                emit_supported += 1;
                o
            }
            Err(droidsaw_dex::emit_dex::DexEmitError::NotImplemented) => {
                emit_not_implemented += 1;
                eprintln!(
                    "  [{}/{total}] NotImplemented: {}",
                    i + 1,
                    path.display()
                );
                continue;
            }
            Err(e) => panic!("emit_dex_collect failed on {}: {e}", path.display()),
        };

        let dex2 = DexFile::parse(&out.bytes, None).unwrap_or_else(|e| {
            panic!("emitted bytes failed to re-parse for {}: {e:?}", path.display())
        });

        // Content equivalence via the quotient newtype — single
        // source of truth for "what counts as round-trip
        // equivalent" lives in parser.rs::ContentEquiv. First
        // divergence halts the run with the offending path.
        assert_eq!(
            ContentEquiv(&dex1),
            ContentEquiv(&dex2),
            "content equivalence violated at {}",
            path.display()
        );

        // Attribution contract: byte-identity ⇒ empty
        // applied_transformations. Violation is an observation bug
        // (emit reported a byte-changing transform fired, yet bytes
        // match) — fail loudly with the offending path.
        let is_byte_identical = bytes == out.bytes;
        if is_byte_identical && !out.applied_transformations.is_empty() {
            panic!(
                "attribution bug at {}: byte-identical output reports \
                 applied transforms {:?}",
                path.display(),
                out.applied_transformations
            );
        }
        if is_byte_identical {
            byte_identical += 1;
        }
        if out.applied_transformations.is_empty() {
            no_reported_transforms += 1;
        }

        if (i + 1) % 25 == 0 {
            eprintln!(
                "  [{}/{total}] checkpoint: {emit_supported} emit-supported, \
                 {emit_not_implemented} NotImplemented, \
                 {byte_identical} byte-identical, \
                 {no_reported_transforms} with no reported transforms",
                i + 1
            );
        }
    }

    let byte_id_pct = if emit_supported > 0 {
        100.0 * (byte_identical as f64) / (emit_supported as f64)
    } else {
        0.0
    };
    let no_transforms_pct = if emit_supported > 0 {
        100.0 * (no_reported_transforms as f64) / (emit_supported as f64)
    } else {
        0.0
    };
    eprintln!(
        "corpus_emit_roundtrip_smoke complete: {total} DEX files, {emit_supported} \
         emit-supported, {emit_not_implemented} NotImplemented, 0 divergences; \
         byte-identity = {byte_identical}/{emit_supported} = {byte_id_pct:.2}%; \
         empty-transforms = {no_reported_transforms}/{emit_supported} = {no_transforms_pct:.2}%. \
         Gap between 100% empty-transforms and 100% byte-identity is the set \
         of unattributed canonicalizations — each bucket of divergence becomes \
         a candidate follow-up task as attribution grows."
    );
}

/// Multi-mode preserve gauge.
///
/// Per file, parses once and emits 4 times: `default`, `map_list`,
/// `encoded_value`, `both`. Counts byte-identity for each mode + the
/// transforms-list shape distribution. Output is the per-mode rate
/// matrix used by v1 announce to claim "byte-identical roundtrip on
/// X% of corpus".
///
/// Opt in with TWO env vars:
///
/// ```bash
/// DROIDSAW_DEX_CORPUS=/path/to/dex-corpus \
/// DROIDSAW_DEX_CORPUS_PRESERVE_MEASURE=1 \
///     cargo test --test corpus_emit_smoke corpus_preserve_measurement -- --nocapture
/// ```
///
/// Tolerant on per-file failures: parse-fails, emit-errs, and
/// content-equivalence violations are logged + counted, not panicked,
/// so a single bad file doesn't abort the measurement. The `complete:`
/// summary line at the end is what reporting consumes.
#[test]
fn corpus_preserve_measurement() {
    if std::env::var("DROIDSAW_DEX_CORPUS_PRESERVE_MEASURE").ok().as_deref() != Some("1") {
        eprintln!(
            "corpus_preserve_measurement: DROIDSAW_DEX_CORPUS_PRESERVE_MEASURE != 1; \
             skipping (set to 1 alongside DROIDSAW_DEX_CORPUS=<dir> to run)"
        );
        return;
    }
    let Some(root) = corpus_root() else {
        eprintln!(
            "corpus_preserve_measurement: DROIDSAW_DEX_CORPUS unset; skipping"
        );
        return;
    };
    let dex_paths = walk_dex_files(&root);
    assert!(
        !dex_paths.is_empty(),
        "corpus root {root:?} has no .dex files"
    );

    let modes: [(&str, EmitConfig); 6] = [
        ("default", EmitConfig::default()),
        (
            "map_list",
            EmitConfig {
                preserve_map_list_order: true,
                ..Default::default()
            },
        ),
        (
            "encoded_value",
            EmitConfig {
                preserve_encoded_value_width: true,
                ..Default::default()
            },
        ),
        (
            "both_legacy",
            EmitConfig {
                preserve_map_list_order: true,
                preserve_encoded_value_width: true,
                ..Default::default()
            },
        ),
        (
            "data_layout",
            EmitConfig {
                preserve_data_section_layout: true,
                ..Default::default()
            },
        ),
        (
            "all3",
            EmitConfig {
                preserve_map_list_order: true,
                preserve_encoded_value_width: true,
                preserve_data_section_layout: true,
                ..Default::default()
            },
        ),
    ];

    let total = dex_paths.len();
    let mut emit_supported = 0usize;
    let mut parse_failed = 0usize;
    let mut emit_failed = 0usize;
    // Per-mode counters parallel to `modes`.
    let mut byte_identical = [0usize; 6];
    let mut content_equiv_failed = [0usize; 6];
    // Per-mode byte_diff totals and counters (for both-preserve mode only,
    // since that's the "best we can do" gauge).
    let mut both_total_diff_bytes = 0usize;
    let mut both_max_diff_bytes = 0usize;
    let mut both_zero_diff_count = 0usize;
    // Transforms histogram for both-preserve mode: how often each variant fires
    // (StringPoolReordered, MapListReordered, EncodedValueReencoded, and per-
    // section AlignmentPaddingInserted counts).
    let mut both_strpool_count = 0usize;
    let mut both_maplist_count = 0usize;
    let mut both_encval_count = 0usize;
    let mut both_align_total = 0usize; // total alignment events across all DEX
    let mut both_align_bytes = 0usize; // total alignment-padding bytes across all DEX
    let mut both_empty_transforms_count = 0usize; // "wholly attributed/zero-diff" candidates

    // Per-section diff diagnostic (both-preserve mode only). Maps DEX
    // section type_code → (files-with-section, files-with-divergent-content,
    // total-divergent-bytes-in-this-section-across-corpus, max-divergence-bytes).
    // Content diff is byte-for-byte on input_bytes[input_off..input_off+input_len]
    // vs output_bytes[output_off..output_off+output_len], NOT position-wise on the
    // whole file (which would inflate divergence via cascading shifts).
    use std::collections::BTreeMap;
    let mut section_total = BTreeMap::<u16, usize>::new();
    let mut section_divergent = BTreeMap::<u16, usize>::new();
    let mut section_divergent_bytes = BTreeMap::<u16, usize>::new();
    let mut section_max_diff = BTreeMap::<u16, usize>::new();
    let mut section_only_in_input = BTreeMap::<u16, usize>::new();
    let mut section_only_in_output = BTreeMap::<u16, usize>::new();

    for (i, path) in dex_paths.iter().enumerate() {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(dex1) = DexFile::parse(&bytes, None) else {
            parse_failed += 1;
            continue;
        };
        let mut all_modes_emitted = true;
        for (mi, (_label, cfg)) in modes.iter().enumerate() {
            let out = match emit_dex_collect(&dex1, cfg) {
                Ok(o) => o,
                Err(_) => {
                    if mi == 0 {
                        all_modes_emitted = false;
                    }
                    continue;
                }
            };
            let dex2 = match DexFile::parse(&out.bytes, None) {
                Ok(d) => d,
                Err(_) => {
                    content_equiv_failed[mi] += 1;
                    continue;
                }
            };
            if ContentEquiv(&dex1) != ContentEquiv(&dex2) {
                content_equiv_failed[mi] += 1;
                continue;
            }
            if bytes == out.bytes {
                byte_identical[mi] += 1;
            }
            // For the all-preserves mode (mi == 5 = "all3"): collect diff
            // distribution + transforms histogram + per-section content diff.
            // This is the "best we can do" gauge.
            if mi == 5 {
                // Per-section content diff (input section bytes vs output
                // section bytes at their respective offsets, by type_code).
                let in_ranges = section_ranges(bytes.len(), &dex1.map_entries);
                let out_ranges = section_ranges(out.bytes.len(), &dex2.map_entries);
                let in_by_tc: BTreeMap<u16, (usize, usize)> = in_ranges
                    .iter()
                    .map(|(tc, s, e)| (*tc, (*s, *e)))
                    .collect();
                let out_by_tc: BTreeMap<u16, (usize, usize)> = out_ranges
                    .iter()
                    .map(|(tc, s, e)| (*tc, (*s, *e)))
                    .collect();
                for tc in in_by_tc.keys().chain(out_by_tc.keys()).copied().collect::<std::collections::BTreeSet<u16>>() {
                    let in_range = in_by_tc.get(&tc);
                    let out_range = out_by_tc.get(&tc);
                    match (in_range, out_range) {
                        (Some(&(in_s, in_e)), Some(&(out_s, out_e))) => {
                            *section_total.entry(tc).or_insert(0) += 1;
                            let in_slice = bytes.get(in_s..in_e).unwrap_or(&[]);
                            let out_slice = out.bytes.get(out_s..out_e).unwrap_or(&[]);
                            if in_slice == out_slice {
                                // byte-identical section
                            } else {
                                *section_divergent.entry(tc).or_insert(0) += 1;
                                let len_diff = in_slice.len().abs_diff(out_slice.len());
                                let pos_diff = in_slice
                                    .iter()
                                    .zip(out_slice.iter())
                                    .filter(|(a, b)| a != b)
                                    .count();
                                let section_diff = pos_diff + len_diff;
                                *section_divergent_bytes.entry(tc).or_insert(0) +=
                                    section_diff;
                                let max = section_max_diff.entry(tc).or_insert(0);
                                if section_diff > *max {
                                    *max = section_diff;
                                }
                            }
                        }
                        (Some(_), None) => {
                            *section_only_in_input.entry(tc).or_insert(0) += 1;
                        }
                        (None, Some(_)) => {
                            *section_only_in_output.entry(tc).or_insert(0) += 1;
                        }
                        (None, None) => unreachable!("type_code came from the union"),
                    }
                }
                let diff = bytes
                    .iter()
                    .zip(out.bytes.iter())
                    .filter(|(a, b)| a != b)
                    .count()
                    + bytes.len().abs_diff(out.bytes.len());
                both_total_diff_bytes = both_total_diff_bytes.saturating_add(diff);
                if diff > both_max_diff_bytes {
                    both_max_diff_bytes = diff;
                }
                if diff == 0 {
                    both_zero_diff_count += 1;
                }
                if out.applied_transformations.is_empty() {
                    both_empty_transforms_count += 1;
                }
                for t in &out.applied_transformations {
                    match t {
                        CanonicalTransform::StringPoolReordered => both_strpool_count += 1,
                        CanonicalTransform::MapListReordered => both_maplist_count += 1,
                        CanonicalTransform::EncodedValueReencoded { count } => {
                            both_encval_count += *count as usize
                        }
                        CanonicalTransform::AlignmentPaddingInserted { byte_count, .. } => {
                            both_align_total += 1;
                            both_align_bytes = both_align_bytes.saturating_add(*byte_count as usize);
                        }
                        // Retired variants (DebugInfoStripped) + future variants
                        // (non_exhaustive). Not counted.
                        _ => {}
                    }
                }
            }
        }
        if all_modes_emitted {
            emit_supported += 1;
        } else {
            emit_failed += 1;
        }
        if (i + 1) % 25 == 0 {
            eprintln!(
                "  [{}/{total}] checkpoint: byte_id default={} map_list={} encoded_value={} \
                 both_legacy={} data_layout={} all3={} | all3_avg_diff={}",
                i + 1,
                byte_identical[0],
                byte_identical[1],
                byte_identical[2],
                byte_identical[3],
                byte_identical[4],
                byte_identical[5],
                if emit_supported > 0 {
                    both_total_diff_bytes / emit_supported.max(1)
                } else {
                    0
                },
            );
        }
    }

    eprintln!("\ncorpus_preserve_measurement complete:");
    eprintln!(
        "  inputs: {total} | parse_failed: {parse_failed} | emit_failed: {emit_failed} | \
         emit_supported: {emit_supported}"
    );
    for (mi, (label, _)) in modes.iter().enumerate() {
        let pct = if emit_supported > 0 {
            100.0 * (byte_identical[mi] as f64) / (emit_supported as f64)
        } else {
            0.0
        };
        eprintln!(
            "  mode={label:<16} byte_identity={}/{emit_supported} = {pct:.2}%  \
             content_equiv_fail={}",
            byte_identical[mi], content_equiv_failed[mi]
        );
    }
    eprintln!(
        "\nv1-announce numbers:\n  default                                    → {:.2}% byte-identity\n  \
         both_legacy (map_list + encoded_value)     → {:.2}% byte-identity\n  \
         data_layout only                            → {:.2}% byte-identity\n  \
         all3 (map_list + encoded_value + layout)   → {:.2}% byte-identity   delta = +{:.2} pts",
        100.0 * (byte_identical[0] as f64) / (emit_supported.max(1) as f64),
        100.0 * (byte_identical[3] as f64) / (emit_supported.max(1) as f64),
        100.0 * (byte_identical[4] as f64) / (emit_supported.max(1) as f64),
        100.0 * (byte_identical[5] as f64) / (emit_supported.max(1) as f64),
        100.0 * (byte_identical[5] as f64 - byte_identical[0] as f64)
            / (emit_supported.max(1) as f64),
    );

    // Diagnostic: what's the floor under both-preserve mode?
    let denom = emit_supported.max(1) as f64;
    let avg_diff = both_total_diff_bytes as f64 / denom;
    eprintln!(
        "\nboth-preserve diff floor diagnostic ({emit_supported} DEX):\n  \
         total_diff_bytes={both_total_diff_bytes}  avg_diff={avg_diff:.1}  \
         max_diff={both_max_diff_bytes}  zero_diff_files={both_zero_diff_count}\n  \
         empty_transforms_files={both_empty_transforms_count} (transforms list was empty \
         but bytes still differed — that diff is wholly unattributed)\n  \
         transforms histogram (counts across all DEX):\n  \
         - StringPoolReordered: {both_strpool_count}\n  \
         - MapListReordered:    {both_maplist_count} (should be 0 — preserve mode is on)\n  \
         - EncodedValueReencoded: {both_encval_count} (should be 0 — preserve mode is on)\n  \
         - AlignmentPaddingInserted: {both_align_total} events ({both_align_bytes} total bytes)",
    );
    let attributed_bytes = both_align_bytes; // alignment is the only byte-count-bearing variant
    let unattributed = both_total_diff_bytes.saturating_sub(attributed_bytes);
    eprintln!(
        "\nattribution gap: total_diff={both_total_diff_bytes} \
         attributed_by_alignment={attributed_bytes} unattributed={unattributed} \
         ({:.1}% of diff)",
        100.0 * (unattributed as f64) / (both_total_diff_bytes.max(1) as f64),
    );

    eprintln!(
        "\nper-section content diff under both-preserve mode (sorted by total \
         divergent bytes desc):"
    );
    // Sort sections by total divergent bytes descending.
    let mut by_section: Vec<(u16, usize, usize, usize, usize)> = section_total
        .iter()
        .map(|(tc, total)| {
            (
                *tc,
                *total,
                *section_divergent.get(tc).unwrap_or(&0),
                *section_divergent_bytes.get(tc).unwrap_or(&0),
                *section_max_diff.get(tc).unwrap_or(&0),
            )
        })
        .collect();
    by_section.sort_by_key(|b| std::cmp::Reverse(b.3));
    let mut grand_total_section_diff = 0usize;
    for (tc, total, divergent, div_bytes, max) in &by_section {
        grand_total_section_diff += div_bytes;
        let pct = if *total > 0 {
            100.0 * (*divergent as f64) / (*total as f64)
        } else {
            0.0
        };
        let avg_div_bytes = if *divergent > 0 {
            *div_bytes / divergent
        } else {
            0
        };
        eprintln!(
            "  {:24} total_files={total:4} divergent={divergent:4} ({pct:5.1}%) \
             total_div_bytes={div_bytes:>10} avg_per_div_file={avg_div_bytes:>8} \
             max_div_in_file={max}",
            type_code_label(*tc)
        );
    }
    eprintln!(
        "  ----  grand total section-content diff bytes: {grand_total_section_diff} \
         (vs position-wise total {both_total_diff_bytes}; position-wise inflates \
         because cascading shifts make every byte after a shift count as a diff)"
    );
    for (tc, n) in &section_only_in_input {
        eprintln!("  {:24} only-in-input: {n} files", type_code_label(*tc));
    }
    for (tc, n) in &section_only_in_output {
        eprintln!("  {:24} only-in-output: {n} files", type_code_label(*tc));
    }
}

/// Tier-1 v1.x scoping gauge: classifies each corpus DEX by whether
/// its input data-section subsection order matches what droidsaw emit
/// produces under default config, AND diagnoses the "mystery" files —
/// files where emit reported `applied_transformations = []` but bytes
/// still differed.
///
/// Two questions answered in one pass:
///
/// 1. **Layout-alignment classification.** For each corpus DEX, compare
///    the input's data-section subsection emit order (sorted by offset
///    from `dex.map_entries`) against the output's. If they match,
///    byte-identity is achievable for this file *without* the v1.x
///    `preserve_data_section_layout` toggle (assuming no other
///    unattributed transforms fire). If they don't match, this file
///    NEEDS the toggle. Reports counts + the ratio of layout-aligned
///    files that actually achieve byte-identity (a ceiling estimate
///    for the post-v1.x rate).
///
/// 2. **Mystery file diagnosis.** For each file where
///    `applied_transformations.is_empty()` but `input_bytes !=
///    output_bytes`, capture the path, the first byte offset that
///    differs, and which section contains it. Surfaces canonicalizations
///    droidsaw is performing without attributing.
///
/// Opt in:
///
/// ```bash
/// DROIDSAW_DEX_CORPUS=/path/to/dex-corpus \
/// DROIDSAW_DEX_CORPUS_TIER1_DIAGNOSTIC=1 \
///     cargo test --release --test corpus_emit_smoke corpus_tier1_diagnostic \
///         -- --nocapture --test-threads=1
/// ```
#[test]
fn corpus_tier1_diagnostic() {
    if std::env::var("DROIDSAW_DEX_CORPUS_TIER1_DIAGNOSTIC").ok().as_deref() != Some("1") {
        eprintln!(
            "corpus_tier1_diagnostic: DROIDSAW_DEX_CORPUS_TIER1_DIAGNOSTIC != 1; skipping"
        );
        return;
    }
    let Some(root) = corpus_root() else {
        eprintln!("corpus_tier1_diagnostic: DROIDSAW_DEX_CORPUS unset; skipping");
        return;
    };
    let dex_paths = walk_dex_files(&root);
    assert!(!dex_paths.is_empty(), "corpus root has no .dex files");

    // Use default config (NOT the preserve toggles) — we want to know how
    // often droidsaw's natural emit order matches input's natural order
    // *before* layout preservation lands.
    let cfg = EmitConfig::default();

    // Subsection set = data-section subsections (everything except the
    // header pools that are always at fixed offsets). For classification,
    // we compare the sub-ORDERING of the data-section subset.
    fn is_data_section(tc: u16) -> bool {
        !matches!(
            tc,
            x if x == map_type::HEADER_ITEM
                || x == map_type::STRING_ID_ITEM
                || x == map_type::TYPE_ID_ITEM
                || x == map_type::PROTO_ID_ITEM
                || x == map_type::FIELD_ID_ITEM
                || x == map_type::METHOD_ID_ITEM
                || x == map_type::CLASS_DEF_ITEM
                || x == map_type::CALL_SITE_ID_ITEM
                || x == map_type::METHOD_HANDLE_ITEM
                || x == map_type::MAP_LIST
        )
    }

    fn data_section_order(entries: &[MapEntry]) -> Vec<u16> {
        let mut sorted = entries.to_vec();
        sorted.sort_by_key(|e| e.offset);
        sorted
            .into_iter()
            .filter(|e| is_data_section(e.type_code))
            .map(|e| e.type_code)
            .collect()
    }

    let total = dex_paths.len();
    let mut parse_failed = 0usize;
    let mut emit_failed = 0usize;
    let mut layout_aligned = 0usize;
    let mut layout_misaligned = 0usize;
    let mut layout_aligned_and_byte_id = 0usize;
    let mut layout_aligned_but_not_byte_id = 0usize;
    let mut layout_misaligned_but_byte_id = 0usize;

    let mut mystery_files: Vec<(String, usize, u16)> = Vec::new(); // (path, first_diff_pos, section_tc)
    let mut empty_transforms_total = 0usize;

    for (i, path) in dex_paths.iter().enumerate() {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(dex1) = DexFile::parse(&bytes, None) else {
            parse_failed += 1;
            continue;
        };
        let out = match emit_dex_collect(&dex1, &cfg) {
            Ok(o) => o,
            Err(_) => {
                emit_failed += 1;
                continue;
            }
        };
        let Ok(dex2) = DexFile::parse(&out.bytes, None) else {
            continue;
        };
        let in_order = data_section_order(&dex1.map_entries);
        let out_order = data_section_order(&dex2.map_entries);
        let byte_identical = bytes == out.bytes;
        if in_order == out_order {
            layout_aligned += 1;
            if byte_identical {
                layout_aligned_and_byte_id += 1;
            } else {
                layout_aligned_but_not_byte_id += 1;
            }
        } else {
            layout_misaligned += 1;
            if byte_identical {
                layout_misaligned_but_byte_id += 1;
            }
        }
        // Mystery files: empty transforms but bytes differ.
        if out.applied_transformations.is_empty() && !byte_identical {
            empty_transforms_total += 1;
            // Find first diff position + which section it falls in.
            let first_diff = bytes
                .iter()
                .zip(out.bytes.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(bytes.len().min(out.bytes.len()));
            let ranges = {
                let mut sorted = dex1.map_entries.clone();
                sorted.sort_by_key(|e| e.offset);
                sorted
                    .iter()
                    .enumerate()
                    .map(|(idx, e)| {
                        let start = e.offset as usize;
                        let end = sorted
                            .get(idx + 1)
                            .map(|n| n.offset as usize)
                            .unwrap_or(bytes.len());
                        (e.type_code, start, end)
                    })
                    .collect::<Vec<_>>()
            };
            let section_tc = ranges
                .iter()
                .find(|(_, s, e)| first_diff >= *s && first_diff < *e)
                .map(|(tc, _, _)| *tc)
                .unwrap_or(0);
            if mystery_files.len() < 20 {
                mystery_files.push((
                    path.display().to_string(),
                    first_diff,
                    section_tc,
                ));
            }
        }

        if (i + 1) % 100 == 0 {
            eprintln!(
                "  [{}/{total}] checkpoint: aligned={layout_aligned} \
                 misaligned={layout_misaligned} mystery={empty_transforms_total}",
                i + 1
            );
        }
    }

    let denom = (layout_aligned + layout_misaligned).max(1) as f64;
    let aligned_pct = 100.0 * (layout_aligned as f64) / denom;
    let misaligned_pct = 100.0 * (layout_misaligned as f64) / denom;
    let aligned_byte_id_pct = if layout_aligned > 0 {
        100.0 * (layout_aligned_and_byte_id as f64) / (layout_aligned as f64)
    } else {
        0.0
    };
    eprintln!(
        "\ncorpus_tier1_diagnostic complete:\n  \
         inputs: {total} | parse_failed: {parse_failed} | emit_failed: {emit_failed}\n\
         \n  --- layout alignment classification ---\n  \
         layout_aligned (input data-section order == emit output order): {layout_aligned} ({aligned_pct:.2}%)\n  \
         layout_misaligned (would need v1.x preserve_data_section_layout toggle): {layout_misaligned} ({misaligned_pct:.2}%)\n  \
         \n  --- byte-identity within each class ---\n  \
         aligned AND byte-identical:   {layout_aligned_and_byte_id} / {layout_aligned} = {aligned_byte_id_pct:.2}% of aligned files\n  \
         aligned BUT NOT byte-id:      {layout_aligned_but_not_byte_id}  ← gated on transforms BEYOND layout preservation\n  \
         misaligned AND byte-identical: {layout_misaligned_but_byte_id}  ← should be 0 unless multiple canonicalizations cancel out\n  \
         \n  --- ceiling estimate ---\n  \
         If v1.x layout-preservation lands cleanly and ONLY layout is the missing fix, byte-identity rate \n  \
         could reach: layout_aligned_and_byte_id / (aligned + misaligned) = {layout_aligned_and_byte_id} / {} = {:.2}%\n  \
         (predicated on misaligned files reaching the same rate as currently-aligned files achieve)",
        layout_aligned + layout_misaligned,
        100.0 * (layout_aligned_and_byte_id as f64) / denom,
    );

    eprintln!(
        "\n  --- mystery files (empty applied_transformations but bytes still differ) ---\n  \
         total mystery files: {empty_transforms_total} | showing first {}",
        mystery_files.len()
    );
    for (path, pos, tc) in &mystery_files {
        eprintln!(
            "    {} | first_diff_at={pos} (section={})",
            path,
            type_code_label(*tc)
        );
    }
}

/// Triage: walk the corpus under preserve_data_section_layout mode and
/// print the first 20 file paths where content-equivalence fails after
/// emit+re-parse. Helps identify what pattern is breaking under preserve.
///
/// Opt in:
///
/// ```bash
/// DROIDSAW_DEX_CORPUS=/path/to/dex-corpus \
/// DROIDSAW_DEX_CORPUS_CONTENT_EQUIV_TRIAGE=1 \
///     cargo test --release --test corpus_emit_smoke corpus_content_equiv_triage \
///         -- --nocapture --test-threads=1
/// ```
#[test]
fn corpus_content_equiv_triage() {
    if std::env::var("DROIDSAW_DEX_CORPUS_CONTENT_EQUIV_TRIAGE").ok().as_deref() != Some("1") {
        eprintln!("corpus_content_equiv_triage: env var unset; skipping");
        return;
    }
    let Some(root) = corpus_root() else { return };
    let dex_paths = walk_dex_files(&root);
    let cfg = EmitConfig {
        preserve_data_section_layout: true,
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        ..Default::default()
    };
    let mut content_equiv_fail_paths: Vec<(String, &'static str, usize, u32)> = Vec::new();
    let mut emit_err_paths: Vec<(String, String)> = Vec::new();
    let mut byte_identical = 0usize;
    let mut content_ok = 0usize;
    let mut content_ok_diff = 0usize;
    let mut emit_err = 0usize;
    let mut parse_err = 0usize;
    let mut content_equiv_fail = 0usize;
    for (i, path) in dex_paths.iter().enumerate() {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let Ok(dex1) = DexFile::parse(&bytes, None) else {
            parse_err += 1;
            continue;
        };
        let out = match emit_dex_collect(&dex1, &cfg) {
            Ok(o) => o,
            Err(e) => {
                emit_err += 1;
                if emit_err_paths.len() < 10 {
                    emit_err_paths.push((path.display().to_string(), format!("{e:?}")));
                }
                continue;
            }
        };
        match DexFile::parse(&out.bytes, None) {
            Ok(dex2) => {
                if ContentEquiv(&dex1) != ContentEquiv(&dex2) {
                    content_equiv_fail += 1;
                    if content_equiv_fail_paths.len() < 20 {
                        // First differing byte position + section
                        let first_diff = bytes.iter().zip(out.bytes.iter())
                            .position(|(a, b)| a != b).unwrap_or(0);
                        // Find section for this position via input map_entries
                        let mut sorted = dex1.map_entries.clone();
                        sorted.sort_by_key(|e| e.offset);
                        let section_tc = sorted.iter().enumerate()
                            .find(|(idx, e)| {
                                let start = e.offset as usize;
                                let end = sorted.get(idx + 1)
                                    .map(|n| n.offset as usize)
                                    .unwrap_or(bytes.len());
                                first_diff >= start && first_diff < end
                            })
                            .map(|(_, e)| e.type_code)
                            .unwrap_or(0);
                        content_equiv_fail_paths.push((
                            path.display().to_string(),
                            "content_equiv_fail",
                            first_diff,
                            u32::from(section_tc),
                        ));
                    }
                } else if bytes == out.bytes {
                    byte_identical += 1;
                    content_ok += 1;
                } else {
                    content_ok += 1;
                    content_ok_diff += 1;
                }
            }
            Err(_) => content_equiv_fail += 1,
        }
        if (i + 1) % 100 == 0 {
            eprintln!("  [{}/{}] checkpoint: byte_id={byte_identical} content_ok={content_ok} \
                      content_fail={content_equiv_fail} emit_err={emit_err}", i + 1, dex_paths.len());
        }
    }
    eprintln!(
        "\ncorpus_content_equiv_triage complete:\n  \
         byte_identical={byte_identical} content_ok_but_diff={content_ok_diff} \
         content_equiv_fail={content_equiv_fail} emit_err={emit_err} parse_err={parse_err}\n"
    );
    eprintln!("--- first {} content_equiv_fail paths (with first-diff section) ---",
              content_equiv_fail_paths.len());
    for (p, _, pos, tc) in &content_equiv_fail_paths {
        eprintln!("  pos={pos:>8} section_tc={tc:#06x}  {p}");
    }
    eprintln!("\n--- first {} emit_err paths ---", emit_err_paths.len());
    for (p, e) in &emit_err_paths {
        eprintln!("  {e}\n    {p}");
    }
}

/// Idempotency gauge: parse → emit (E1) → parse → emit (E2). Assert E1 == E2.
///
/// Tests whether the emit pipeline is deterministic and idempotent. If it is,
/// droidsaw produces a well-defined canonical DEX form, and the claim
/// "round-trip through droidsaw-canonical form is byte-identical" is real
/// — a defensible v1 announce strengthening even without fixing the
/// data-section-layout preservation gap.
///
/// Opt in:
///
/// ```bash
/// DROIDSAW_DEX_CORPUS=/path/to/dex-corpus \
/// DROIDSAW_DEX_CORPUS_IDEMPOTENCY_MEASURE=1 \
///     cargo test --release --test corpus_emit_smoke corpus_emit_idempotency \
///         -- --nocapture --test-threads=1
/// ```
#[test]
fn corpus_emit_idempotency() {
    if std::env::var("DROIDSAW_DEX_CORPUS_IDEMPOTENCY_MEASURE").ok().as_deref() != Some("1") {
        eprintln!(
            "corpus_emit_idempotency: DROIDSAW_DEX_CORPUS_IDEMPOTENCY_MEASURE != 1; skipping"
        );
        return;
    }
    let Some(root) = corpus_root() else {
        eprintln!("corpus_emit_idempotency: DROIDSAW_DEX_CORPUS unset; skipping");
        return;
    };
    let dex_paths = walk_dex_files(&root);
    assert!(!dex_paths.is_empty(), "corpus root has no .dex files");

    let cfg = EmitConfig::default();

    let total = dex_paths.len();
    let mut parse_failed_input = 0usize;
    let mut emit_failed_e1 = 0usize;
    let mut parse_failed_e1 = 0usize;
    let mut emit_failed_e2 = 0usize;
    let mut idempotent = 0usize;
    let mut non_idempotent = 0usize;
    let mut total_e1_e2_diff_bytes = 0usize;
    let mut max_e1_e2_diff_bytes = 0usize;

    for (i, path) in dex_paths.iter().enumerate() {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(dex1) = DexFile::parse(&bytes, None) else {
            parse_failed_input += 1;
            continue;
        };
        let e1 = match emit_dex_collect(&dex1, &cfg) {
            Ok(o) => o.bytes,
            Err(_) => {
                emit_failed_e1 += 1;
                continue;
            }
        };
        let Ok(dex2) = DexFile::parse(&e1, None) else {
            parse_failed_e1 += 1;
            continue;
        };
        let e2 = match emit_dex_collect(&dex2, &cfg) {
            Ok(o) => o.bytes,
            Err(_) => {
                emit_failed_e2 += 1;
                continue;
            }
        };
        if e1 == e2 {
            idempotent += 1;
        } else {
            non_idempotent += 1;
            let diff = e1
                .iter()
                .zip(e2.iter())
                .filter(|(a, b)| a != b)
                .count()
                + e1.len().abs_diff(e2.len());
            total_e1_e2_diff_bytes = total_e1_e2_diff_bytes.saturating_add(diff);
            if diff > max_e1_e2_diff_bytes {
                max_e1_e2_diff_bytes = diff;
            }
        }
        if (i + 1) % 50 == 0 {
            eprintln!(
                "  [{}/{total}] checkpoint: idempotent={idempotent} \
                 non_idempotent={non_idempotent}",
                i + 1
            );
        }
    }

    let denom = (idempotent + non_idempotent).max(1) as f64;
    let pct = 100.0 * (idempotent as f64) / denom;
    eprintln!(
        "\ncorpus_emit_idempotency complete:\n  \
         inputs: {total} | parse_failed_input: {parse_failed_input} | \
         emit_failed_e1: {emit_failed_e1} | parse_failed_e1: {parse_failed_e1} | \
         emit_failed_e2: {emit_failed_e2}\n  \
         idempotent: {idempotent}/{}=  {pct:.2}%   \
         non_idempotent: {non_idempotent}\n  \
         non-idempotent E1↔E2 diff: total={total_e1_e2_diff_bytes} \
         max_in_file={max_e1_e2_diff_bytes} \
         avg_per_non_idempotent_file={}",
        idempotent + non_idempotent,
        total_e1_e2_diff_bytes.checked_div(non_idempotent).unwrap_or(0),
    );
}

/// Tier-ladder triage under all-preserve mode.
///
/// Under `preserve_data_section_layout` + `preserve_map_list_order` +
/// `preserve_encoded_value_width` (all 3 on), droidsaw claims to
/// preserve every section's bytes verbatim. Any non-byte-identical
/// output therefore falls into a fixed ladder:
///
/// - **Tier 0**: byte-identical. No bug.
/// - **Tier 1**: `content_equiv_FAIL` — output reparses to a different
///   `ContentEquiv`. Real parser/emit bug (or upstream-cascaded one).
/// - **Tier 2**: bytes diverge in a section whose bytes we claim to
///   preserve (everything except header / map_list). A bug we hadn't
///   yet attributed.
/// - **Tier 4**: only `header` and/or `map_list` bytes diverge. Cascade
///   from derived fields (Adler-32, SHA-1, file_size, map_off, per-
///   section offsets). Not a bug — surfaces upstream Tier 1/2 fixes
///   automatically.
///
/// Tier 3 (intentional canonicalization with `CanonicalTransform`
/// attribution) doesn't apply in all-preserve mode — we promised
/// everything.
///
/// Opt in:
///
/// ```bash
/// DROIDSAW_DEX_CORPUS=/path/to/dex-corpus \
/// DROIDSAW_DEX_CORPUS_TIER_LADDER=1 \
///     cargo test --release --test corpus_emit_smoke corpus_tier_ladder \
///         -- --nocapture --test-threads=1
/// ```
#[test]
fn corpus_tier_ladder() {
    if std::env::var("DROIDSAW_DEX_CORPUS_TIER_LADDER").ok().as_deref() != Some("1") {
        eprintln!(
            "corpus_tier_ladder: DROIDSAW_DEX_CORPUS_TIER_LADDER != 1; skipping"
        );
        return;
    }
    let Some(root) = corpus_root() else {
        eprintln!("corpus_tier_ladder: DROIDSAW_DEX_CORPUS unset; skipping");
        return;
    };
    let dex_paths = walk_dex_files(&root);
    assert!(!dex_paths.is_empty(), "corpus root has no .dex files");

    // preserve_input_checksums lets the test close inputs with
    // non-canonical Adler/SHA in their headers (some real-world DEX
    // files have this). The flag produces non-loadable DEX when
    // the input was non-canonical — fine for byte-identity comparison;
    // production emit must never set it.
    let cfg = EmitConfig {
        preserve_data_section_layout: true,
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        preserve_input_checksums: true,
        ..Default::default()
    };

    let total = dex_paths.len();
    let mut tier_0_byte_id = 0usize;
    let mut tier_1_content_fail = 0usize;
    let mut tier_2_section_diverge = 0usize;
    let mut tier_4_cascade_only = 0usize;
    let mut parse_err = 0usize;
    let mut emit_err = 0usize;
    let mut reparse_err = 0usize;

    // Per-tier exemplar paths (cap to keep the report readable).
    let mut tier_1_paths: Vec<String> = Vec::new();
    let mut tier_2_paths: Vec<(String, &'static str, u32)> = Vec::new(); // (path, first_section_label, byte_count)
    let mut tier_4_paths: Vec<String> = Vec::new();

    // Per-section Tier 2 byte tally, so we can see which claimed-
    // preserve section is the dominant contributor to remaining work.
    let mut tier_2_by_section: std::collections::BTreeMap<u16, (u32, u32)> =
        std::collections::BTreeMap::new(); // type_code → (file_count, total_diff_bytes)

    for (i, path) in dex_paths.iter().enumerate() {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let Ok(dex1) = DexFile::parse(&bytes, None) else {
            parse_err += 1;
            continue;
        };
        let out = match emit_dex_collect(&dex1, &cfg) {
            Ok(o) => o,
            Err(e) => {
                // Preserve-mode refusal (typed UnrepresentableIR from
                // section_layout_gauge or UnparseableTail) is the expected
                // signal: input can't be faithfully round-tripped under
                // preserve mode. Bucket as Tier 2
                // — "preserve unattainable" — not generic emit_err.
                let msg = format!("{e:?}");
                if msg.contains("preserve_data_section_layout") {
                    tier_2_section_diverge += 1;
                    if tier_2_paths.len() < 30 {
                        tier_2_paths.push((
                            format!("[preserve_refused] {}", path.display()),
                            "preserve_refused",
                            0,
                        ));
                    }
                } else {
                    emit_err += 1;
                }
                continue;
            }
        };
        if bytes == out.bytes {
            tier_0_byte_id += 1;
            continue;
        }
        let dex2 = match DexFile::parse(&out.bytes, None) {
            Ok(d) => d,
            Err(_) => {
                // Reparse failure is a SEVERE Tier 1: not just "output
                // IR differs from input IR" — output isn't even valid
                // DEX. Roll into Tier 1 so the punch list reflects all
                // semantic bugs in one place.
                reparse_err += 1;
                tier_1_content_fail += 1;
                if tier_1_paths.len() < 20 {
                    tier_1_paths.push(format!("[reparse_err] {}", path.display()));
                }
                continue;
            }
        };
        if ContentEquiv(&dex1) != ContentEquiv(&dex2) {
            tier_1_content_fail += 1;
            if tier_1_paths.len() < 20 {
                tier_1_paths.push(path.display().to_string());
            }
            continue;
        }

        // Content-equivalent but bytes differ → Tier 2 or Tier 4.
        // Classify by which sections carry the diff. Use input map for
        // section ranges (output map should match under preserve).
        let ranges = section_ranges(bytes.len().min(out.bytes.len()), &dex1.map_entries);
        let len = bytes.len().min(out.bytes.len());
        let mut nonheader_sections_with_diff: Vec<(u16, u32)> = Vec::new();
        let mut header_or_maplist_diff = false;
        for (tc, start, end) in &ranges {
            let s = *start;
            let e = (*end).min(len);
            if s >= e {
                continue;
            }
            let diff_bytes: u32 = bytes[s..e]
                .iter()
                .zip(out.bytes[s..e].iter())
                .filter(|(a, b)| a != b)
                .count()
                .try_into()
                .unwrap_or(u32::MAX);
            if diff_bytes == 0 {
                continue;
            }
            if *tc == map_type::HEADER_ITEM || *tc == map_type::MAP_LIST {
                header_or_maplist_diff = true;
            } else {
                nonheader_sections_with_diff.push((*tc, diff_bytes));
            }
        }

        if nonheader_sections_with_diff.is_empty() && header_or_maplist_diff {
            tier_4_cascade_only += 1;
            if tier_4_paths.len() < 20 {
                tier_4_paths.push(path.display().to_string());
            }
        } else if !nonheader_sections_with_diff.is_empty() {
            tier_2_section_diverge += 1;
            // Tally per-section bytes.
            for (tc, db) in &nonheader_sections_with_diff {
                let entry = tier_2_by_section.entry(*tc).or_insert((0, 0));
                entry.0 = entry.0.saturating_add(1);
                entry.1 = entry.1.saturating_add(*db);
            }
            if tier_2_paths.len() < 30 {
                // Tag with the first (= lowest type_code in the
                // BTreeMap; for diagnostics it's the first hit in the
                // sorted-by-offset walk).
                let (first_tc, first_db) = nonheader_sections_with_diff[0];
                tier_2_paths.push((
                    path.display().to_string(),
                    type_code_label(first_tc),
                    first_db,
                ));
            }
        } else {
            // Bytes differ but no section bucket caught it — file size
            // mismatch entirely outside any map entry. Lump into Tier 4.
            tier_4_cascade_only += 1;
        }

        if (i + 1) % 100 == 0 {
            eprintln!(
                "  [{}/{}] t0={tier_0_byte_id} t1={tier_1_content_fail} \
                 t2={tier_2_section_diverge} t4={tier_4_cascade_only} \
                 parse_err={parse_err} emit_err={emit_err}",
                i + 1,
                dex_paths.len()
            );
        }
    }

    // Reparse-failed files are counted in Tier 1 (severe) — exclude
    // only true input-parse / emit failures from the denominator.
    let denominator = total.saturating_sub(parse_err + emit_err);
    eprintln!("\n=== Tier ladder (all-preserve mode) — {total} files ===");
    eprintln!(
        "  Tier 0 (byte_id)           : {tier_0_byte_id:>5} / {denominator}  ({:.2}%)",
        100.0 * (tier_0_byte_id as f64) / (denominator as f64).max(1.0)
    );
    eprintln!(
        "  Tier 1 (content_equiv_FAIL): {tier_1_content_fail:>5} / {denominator}  ({:.2}%)",
        100.0 * (tier_1_content_fail as f64) / (denominator as f64).max(1.0)
    );
    eprintln!(
        "  Tier 2 (claimed-preserve)  : {tier_2_section_diverge:>5} / {denominator}  ({:.2}%)",
        100.0 * (tier_2_section_diverge as f64) / (denominator as f64).max(1.0)
    );
    eprintln!(
        "  Tier 4 (cascade-only)      : {tier_4_cascade_only:>5} / {denominator}  ({:.2}%)",
        100.0 * (tier_4_cascade_only as f64) / (denominator as f64).max(1.0)
    );
    eprintln!(
        "  errors: input_parse={parse_err} emit={emit_err}  \
         (of Tier 1: {reparse_err} are reparse failures, rest are content_equiv_FAIL)"
    );

    if !tier_2_by_section.is_empty() {
        eprintln!("\n=== Tier 2 contribution by section ===");
        for (tc, (count, bytes)) in &tier_2_by_section {
            eprintln!(
                "  {:>24}  files={count:>4}  total_diff_bytes={bytes}",
                type_code_label(*tc)
            );
        }
    }

    if !tier_1_paths.is_empty() {
        eprintln!("\n=== Tier 1 exemplars (first {}) ===", tier_1_paths.len());
        for p in &tier_1_paths {
            eprintln!("  {p}");
        }
    }
    if !tier_2_paths.is_empty() {
        eprintln!("\n=== Tier 2 exemplars (first {}) ===", tier_2_paths.len());
        for (p, label, db) in &tier_2_paths {
            eprintln!("  section={label:<24}  diff_bytes={db:>6}  {p}");
        }
    }
    if !tier_4_paths.is_empty() {
        eprintln!("\n=== Tier 4 exemplars (first {}) ===", tier_4_paths.len());
        for p in &tier_4_paths {
            eprintln!("  {p}");
        }
    }
}
