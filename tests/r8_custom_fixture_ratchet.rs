//! In-tree ground-truth ratchet for the R8 BlockOutline recogniser.
//!
//! Consumes the artifacts at
//! `tests/fixtures/r8/block_outlining/artifacts/{classes.dex,mapping.txt}`
//! produced by `scripts/regen-r8-fixture.sh`. The Kotlin source under
//! `tests/fixtures/r8/block_outlining/source/Main.kt` is engineered to
//! satisfy R8's outliner invariants I4–I13 with 30 distinct caller
//! methods invoking an identical 4-instruction `StringBuilder` body —
//! R8 outlines the shared sequence into a synthetic helper.
//!
//! Because the fixture is in-tree, ground truth is exact: the
//! ratchet asserts that the recogniser fires on the helper (TP) AND
//! does not fire on any developer-side caller (FP-free). No corpus
//! placeholder, no "informative-only" sample-size caveat — the
//! fixture IS the n.
//!
//! # SKIP path
//!
//! The artifacts are NOT checked in as committed binary blobs until
//! the regen pipeline is exercised on a machine with the Android SDK.
//! Before then, the test SKIPs cleanly (no panic, no spurious failure)
//! with a message pointing at the regen script. Once the artifacts
//! are present, the test asserts ground-truth equality.
//!
//! # Pipeline placement
//!
//! Sibling to the env-gated mapping-paired ratchets but with the
//! data path inverted: those ratchets read from analyst-local
//! corpora outside the repo; this ratchet reads from in-tree
//! artifacts. The two are complementary — the mapping-paired
//! ratchets test the recogniser against real-world R8 output on
//! large corpora; this ratchet pins the recogniser's response on
//! a controlled synthetic input we own.

mod common;

use std::path::PathBuf;

use common::r8_canonical_marker::{descriptor_to_mapping_key, parse_block_outlined_marker};
use common::r8_mapping_outline::{OutlineSet, SyntheticKind};

const FIXTURE_REL: &str = "tests/fixtures/r8/block_outlining/artifacts";
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Minimum number of mapping-confirmed outline annotations the
/// fixture must produce. The Kotlin source has THREE caller groups
/// of 30 callers each, with bodies that differ by parameter type
/// (Int, Long, String) — each invokes a different
/// `StringBuilder.append(...)` overload. R8's outliner emits one
/// helper per distinct body signature, so the floor is 3.
///
/// The floor catches "the outliner did not fire on the engineered
/// shape" OR "R8 collapsed our distinct bodies into a single
/// helper" — both are signals worth surfacing. A future R8 version
/// could produce MORE helpers (subdivision) which is also fine; the
/// floor is a lower bound, not an exact count.
const MIN_EXPECTED_OUTLINE_ANNOTATIONS: usize = 3;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_REL)
        .join(name)
}

#[test]
fn r8_custom_fixture_recogniser_matches_in_tree_ground_truth() {
    let handle = std::thread::Builder::new()
        .name("r8_custom_fixture_worker".into())
        .stack_size(SMOKE_TEST_STACK_BYTES)
        .spawn(fixture_main)
        .expect("spawn stack-sized worker thread");
    if let Err(e) = handle.join() {
        std::panic::resume_unwind(e);
    }
}

fn fixture_main() {
    let dex_path = fixture_path("classes.dex");
    let mapping_path = fixture_path("mapping.txt");

    if !dex_path.is_file() || !mapping_path.is_file() {
        eprintln!(
            "SKIP: in-tree R8 fixture artifacts not present at {}. \
             Run tests/fixtures/r8/block_outlining/scripts/regen-r8-fixture.sh \
             to generate classes.dex + mapping.txt from the engineered Kotlin \
             source. Requires Android SDK build-tools (r8 / d8) + kotlinc.",
            dex_path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
        );
        return;
    }

    // Mapping — ground truth.
    let outlines = match OutlineSet::from_file(&mapping_path) {
        Ok(s) => s,
        Err(e) => {
            panic!(
                "in-tree fixture mapping.txt failed to parse: {e} \
                 (path: {}). Re-run the regen script.",
                mapping_path.display(),
            );
        }
    };

    let annotation_count = outlines.outlined_count();
    assert!(
        annotation_count >= MIN_EXPECTED_OUTLINE_ANNOTATIONS,
        "in-tree fixture mapping has {} com.android.tools.r8.outline annotations, \
         expected >= {}. The engineered 30-caller pattern in source/Main.kt no \
         longer satisfies R8's outliner predicate, or the R8 version bumped its \
         threshold. Re-derive the pattern (see I4-I13 in r8_inversion).",
        annotation_count, MIN_EXPECTED_OUTLINE_ANNOTATIONS,
    );

    let data = std::fs::read(&dex_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", dex_path.display()));
    let dex = droidsaw_dex::parser::DexFile::parse(&data, None)
        .unwrap_or_else(|e| panic!("parse {}: {e:?}", dex_path.display()));
    let census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);

    // Walk every class, decompile, collect markers. Record per-marker
    // (mapping-key class, method) tuples for later ground-truth
    // intersection with the mapping.
    let mut marker_tuples: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    let mut total_markers = 0usize;
    let mut total_classes = 0usize;
    for class_def in &dex.class_defs {
        if class_def.class_data_off == 0 {
            continue;
        }
        total_classes = total_classes.saturating_add(1);
        let out = droidsaw_dex::classes::decompile_class_with_census(
            &dex, &data, class_def, &census,
        );
        for line in out.lines() {
            let Some(marker) = parse_block_outlined_marker(line) else {
                continue;
            };
            let key_class = descriptor_to_mapping_key(marker.helper_class);
            if key_class.is_empty() {
                continue;
            }
            total_markers = total_markers.saturating_add(1);
            marker_tuples.insert((key_class, marker.helper_method.to_string()));
        }
    }

    // Ground-truth intersection. Recogniser TPs = markers that match
    // a mapping outline tuple. FPs = markers on (class, method)
    // tuples NOT in the mapping outline set.
    let mut tp_with_kind: std::collections::BTreeMap<SyntheticKind, usize> =
        std::collections::BTreeMap::new();
    let mut fp_tuples: Vec<(String, String)> = Vec::new();
    for (class, method) in &marker_tuples {
        match outlines.outlined_kind(class, method) {
            Some(kind) => {
                let slot = tp_with_kind.entry(kind).or_insert(0);
                *slot = slot.saturating_add(1);
            }
            None => {
                fp_tuples.push((class.clone(), method.clone()));
            }
        }
    }

    eprintln!(
        "CUSTOM FIXTURE RATCHET: {} classes, {} markers, {} distinct (class, method) marker tuples",
        total_classes,
        total_markers,
        marker_tuples.len(),
    );
    eprintln!(
        "  GROUND TRUTH: {} outline annotations in mapping",
        annotation_count,
    );
    eprintln!("  PER-KIND TP BREAKDOWN:");
    for (kind, n) in &tp_with_kind {
        eprintln!("    {:<32} {}", kind.label(), n);
    }
    if !fp_tuples.is_empty() {
        eprintln!("  FPs (recogniser fired, mapping says NOT outlined):");
        for (c, m) in fp_tuples.iter().take(10) {
            eprintln!("    {c}.{m}");
        }
    }

    // Ground-truth assertions — the whole point of an in-tree fixture.
    // 1. Recogniser MUST fire at least once on the engineered helper.
    assert!(
        !marker_tuples.is_empty(),
        "recogniser fired ZERO markers on the in-tree fixture despite \
         the mapping having {annotation_count} outline annotation(s). \
         The recogniser's I4-I13 predicates have drifted away from what \
         R8 actually emits — re-derive against the mapping ground truth.",
    );
    // 2. Recogniser MUST NOT fire on any tuple the mapping disagrees
    //    with. Unlike the analyst-local mapping-paired ratchets,
    //    there's no FP_ALLOWLIST escape hatch here — the fixture is
    //    engineered, so disagreement is recogniser drift, full stop.
    assert!(
        fp_tuples.is_empty(),
        "recogniser fired on {} (class, method) tuple(s) that the \
         mapping does NOT declare outlined. The engineered fixture has \
         no allowlist — this is unambiguous recogniser drift. First FPs:\n  {}",
        fp_tuples.len(),
        fp_tuples
            .iter()
            .take(5)
            .map(|(c, m)| format!("{c}.{m}"))
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}
