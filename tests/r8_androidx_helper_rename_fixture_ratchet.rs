//! In-tree ratchet for the R8 BlockOutline recogniser FP demo on
//! AndroidX-style minified library helpers.
//!
//! Consumes the artifacts at
//! `tests/fixtures/r8/androidx_helper_rename/artifacts/{classes.dex,mapping.txt}`
//! produced by `scripts/regen-androidx-helper-fixture.sh`. The Kotlin
//! source under `tests/fixtures/r8/androidx_helper_rename/source/Main.kt`
//! is engineered to make R8 minify the `androidx.testlib.*` helper
//! class to short names WITHOUT firing the outliner — every helper
//! body is structurally distinct from the others, so R8's >= 20
//! distinct callers per identical body predicate cannot match.
//!
//! The fixture is the empirical anchor for a proposed `androidx`
//! family entry in the recogniser's FP-suppression list. It is the
//! AndroidX-named sibling of `tests/fixtures/r8/library_helper_rename/`
//! (Flutter-style, neutral package). Parallel evidence from two
//! ecosystems supports the family-shaped FP hypothesis: library
//! helpers minified to short names happen to satisfy I4-I13
//! invariants; R8 doesn't outline them, just renames them.
//!
//! # SKIP path
//!
//! Artifacts are NOT checked in until the regen pipeline is exercised
//! on a machine with the Android SDK + kotlinc. Before that, the test
//! SKIPs cleanly (no panic, no spurious failure) with a message
//! pointing at the regen script.
//!
//! # Ground-truth shape
//!
//! 1. Mapping MUST contain at least one class rename inside the
//!    `androidx.testlib.*` namespace. Evidence R8 actually minified
//!    the helpers — if absent, an unintended `-keep` rule preserved
//!    the names and the FP demo is hollow.
//! 2. Mapping MUST NOT contain any `com.android.tools.r8.outline`
//!    annotations on tuples whose original class lives under
//!    `androidx.testlib.*`. The helpers are renamed, not outlined.
//! 3. Recogniser MUST fire on at least one renamed `androidx.testlib.*`
//!    class — that's the FP demo.
//! 4. Every recogniser-fired tuple MUST be absent from the mapping's
//!    OutlineSet — the recogniser is producing structural false
//!    positives, and the mapping is the ground truth saying so.

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use common::r8_canonical_marker::{descriptor_to_mapping_key, parse_block_outlined_marker};
use common::r8_mapping_outline::OutlineSet;

const FIXTURE_REL: &str = "tests/fixtures/r8/androidx_helper_rename/artifacts";
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

/// The stub-marker namespace the fixture's Kotlin source uses. Keep
/// in sync with `source/Main.kt`'s `package androidx.testlib`
/// declaration. We intentionally use `testlib` (not a real AndroidX
/// subnamespace like `lifecycle` or `appcompat`) so the fixture
/// cannot collide with a genuine AndroidX dependency in any
/// future fuzz / corpus interaction.
const ANDROIDX_STUB_PREFIX: &str = "androidx.testlib.";

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_REL)
        .join(name)
}

#[test]
fn r8_androidx_helper_rename_fp_demo() {
    let handle = std::thread::Builder::new()
        .name("r8_androidx_helper_rename_worker".into())
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
            "SKIP: AndroidX library-helper rename fixture artifacts not present at {}. \
             Run tests/fixtures/r8/androidx_helper_rename/scripts/regen-androidx-helper-fixture.sh \
             to generate classes.dex + mapping.txt from the engineered Kotlin \
             source. Requires Android SDK build-tools (r8 / d8) + kotlinc.",
            dex_path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
        );
        return;
    }

    let mapping_text = std::fs::read_to_string(&mapping_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", mapping_path.display()));

    // (1) Evidence R8 minified the androidx.testlib.* helpers.
    let androidx_renames = collect_androidx_testlib_renames(&mapping_text);
    assert!(
        !androidx_renames.is_empty(),
        "mapping.txt has zero `{ANDROIDX_STUB_PREFIX}*` class-rename records. \
         Either R8 did not minify (an unintended -keep rule may have leaked \
         in), or the source/Main.kt package was changed. The FP demo cannot \
         proceed without evidence that R8 renamed the helpers."
    );
    // Each rename: (original_androidx_class, obfuscated_class).
    let obfuscated_androidx_classes: BTreeSet<String> = androidx_renames
        .iter()
        .map(|(_, obf)| obf.clone())
        .collect();

    // (2) Mapping MUST be outline-FREE on androidx.testlib.* tuples.
    // Parse the full OutlineSet and check that no outlined-method
    // tuple's original class lives under the stub prefix. Because
    // OutlineSet stores OBFUSCATED tuples and we have the
    // original->obfuscated map for androidx.testlib.*, intersect.
    let outlines = OutlineSet::from_file(&mapping_path).unwrap_or_else(|e| {
        panic!(
            "mapping.txt failed to parse as OutlineSet: {e} \
             (path: {}). Re-run the regen script.",
            mapping_path.display(),
        )
    });
    let outline_intersect_androidx: Vec<(String, String)> = outlines
        .outlined_methods()
        .filter(|(c, _)| obfuscated_androidx_classes.contains(*c))
        .map(|(c, m)| (c.to_string(), m.to_string()))
        .collect();
    assert!(
        outline_intersect_androidx.is_empty(),
        "mapping.txt declares {} com.android.tools.r8.outline annotation(s) \
         on `{ANDROIDX_STUB_PREFIX}*` tuples. The fixture is engineered to be \
         outline-FREE — the source/Main.kt helpers should be structurally \
         distinct enough that R8 cannot outline them. If R8 did outline, the \
         FP demo is invalidated; re-engineer source/Main.kt for more body \
         diversity. First offending tuples:\n  {}",
        outline_intersect_androidx.len(),
        outline_intersect_androidx
            .iter()
            .take(5)
            .map(|(c, m)| format!("{c}.{m}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    // (3) Recogniser MUST fire on at least one renamed androidx.testlib.*
    // class — the structural FP demo.
    let data = std::fs::read(&dex_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", dex_path.display()));
    let dex = droidsaw_dex::parser::DexFile::parse(&data, None)
        .unwrap_or_else(|e| panic!("parse {}: {e:?}", dex_path.display()));
    let census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);

    let mut marker_tuples: BTreeSet<(String, String)> = BTreeSet::new();
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

    // Subset of marker tuples that landed on an androidx.testlib.*
    // (post-rename) class. These are the structural FPs the family
    // entry would suppress.
    let androidx_marker_tuples: Vec<(String, String)> = marker_tuples
        .iter()
        .filter(|(c, _)| obfuscated_androidx_classes.contains(c))
        .cloned()
        .collect();

    eprintln!(
        "ANDROIDX HELPER-RENAME RATCHET: {} classes, {} markers, {} distinct (class, method) marker tuples",
        total_classes,
        total_markers,
        marker_tuples.len(),
    );
    eprintln!(
        "  ANDROIDX RENAME EVIDENCE: {} `{ANDROIDX_STUB_PREFIX}*` class renames in mapping",
        androidx_renames.len(),
    );
    eprintln!(
        "  RECOGNISER FPs ON RENAMED ANDROIDX: {} tuple(s)",
        androidx_marker_tuples.len(),
    );
    for (c, m) in androidx_marker_tuples.iter().take(10) {
        eprintln!("    {c}.{m}");
    }

    assert!(
        !androidx_marker_tuples.is_empty(),
        "recogniser fired ZERO markers on the renamed `{ANDROIDX_STUB_PREFIX}*` \
         classes ({} obfuscated classes inspected). Either the recogniser's \
         I4-I13 predicates no longer match the structural shape of small \
         static library helpers (in which case the FP demo no longer reproduces \
         and the family entry may not be needed), or the helper bodies in \
         source/Main.kt have drifted away from the canonical AndroidX \
         shape. Inspect the decompiler output for one of the obfuscated classes \
         to debug.",
        obfuscated_androidx_classes.len(),
    );

    // (4) KEY ASSERTION: every marker on a renamed androidx.testlib.*
    // tuple MUST be absent from the OutlineSet. The mapping is the
    // ground truth saying "these are not R8 outlines" and the
    // recogniser fired on them anyway — by definition every such
    // tuple is a false positive.
    let mut leaked_into_outline_set: Vec<(String, String)> = Vec::new();
    for (c, m) in &androidx_marker_tuples {
        if outlines.is_outlined(c, m) {
            leaked_into_outline_set.push((c.clone(), m.clone()));
        }
    }
    assert!(
        leaked_into_outline_set.is_empty(),
        "{} recogniser-fired `{ANDROIDX_STUB_PREFIX}*` tuple(s) ARE present in \
         the OutlineSet, contradicting the fixture's outline-FREE invariant \
         enforced at step (2). This indicates either the mapping parser or \
         the rename-collection logic disagrees with the OutlineSet on what \
         constitutes an outlined tuple. Investigate before trusting the FP \
         demo. First offenders:\n  {}",
        leaked_into_outline_set.len(),
        leaked_into_outline_set
            .iter()
            .take(5)
            .map(|(c, m)| format!("{c}.{m}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// Scan mapping.txt text for class-rename records whose ORIGINAL class
/// name lives under `androidx.testlib.*`. Returns
/// (original_class, obfuscated_class) tuples.
///
/// Mapping class records are unindented and have the shape
/// `original.fully.qualified.Class -> obfuscated.Class:`.
fn collect_androidx_testlib_renames(mapping_text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in mapping_text.lines() {
        // Unindented only — method records are indented.
        if line.is_empty() {
            continue;
        }
        let first = line.as_bytes()[0];
        if first == b' ' || first == b'\t' {
            continue;
        }
        let Some(stripped) = line.strip_suffix(':') else {
            continue;
        };
        let Some((orig, obf)) = stripped.split_once(" -> ") else {
            continue;
        };
        if orig.is_empty() || obf.is_empty() {
            continue;
        }
        if !orig.starts_with(ANDROIDX_STUB_PREFIX) {
            continue;
        }
        out.push((orig.to_string(), obf.to_string()));
    }
    out
}
