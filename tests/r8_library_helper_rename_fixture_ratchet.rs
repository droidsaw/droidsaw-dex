//! In-tree counter-example ratchet for the R8 BlockOutline recogniser.
//!
//! The companion to `r8_custom_fixture_ratchet.rs` (which proves the
//! recogniser fires on TRUE outlines). This test consumes the
//! artifacts at
//! `tests/fixtures/r8/library_helper_rename/artifacts/{classes.dex,mapping.txt}`
//! produced by
//! `tests/fixtures/r8/library_helper_rename/scripts/regen-library-helper-fixture.sh`
//! and asserts the structural recogniser's known FALSE POSITIVE
//! behaviour on R8-minified library helper code.
//!
//! The Kotlin source under
//! `tests/fixtures/r8/library_helper_rename/source/Main.kt` mimics
//! the shape of flutter_embedding's helper classes (e.g.
//! `io.flutter.plugin.platform.PlatformPlugin`): 24 short static
//! methods on a single container, each with a straight-line body of
//! a few invokes and a return, each invoked from multiple call sites.
//! Bodies differ STRUCTURALLY (different invokes per helper) so R8's
//! outliner does NOT extract a shared body. R8's MINIFIER still
//! renames the container + methods to short names, per the
//! `allowshrinking,allowobfuscation` keep rule (mirrors Flutter's
//! `packages/flutter_tools/gradle/flutter_proguard_rules.pro` shape).
//!
//! # The DOCUMENTED FP family
//!
//! This fixture is the empirical anchor for the `io.flutter` entry
//! in the BlockOutline known-FP family list. The assertion path:
//!
//!   1. Mapping shows class renames on MyLibraryHelpers and its
//!      methods (R8's minifier ran).
//!   2. Mapping carries ZERO `com.android.tools.r8.outline`
//!      annotations on the MyLibraryHelpers slice (R8's outliner did
//!      NOT extract these as synthetic helpers).
//!   3. The structural recogniser walks the DEX and FIRES at least
//!      one marker on the renamed helper class (because the
//!      structural predicates I4-I13 do trigger on short-named,
//!      static, straight-line methods with many callers).
//!   4. Every marker fired on the helper slice resolves to
//!      `OutlineSet::outlined_kind() == None` (mapping disagrees).
//!
//! Steps (3) + (4) together are the documented FP: the recogniser
//! cannot distinguish "synthesized outline" from "R8-renamed library
//! helper" from DEX shape alone — the mapping is the disambiguator.
//! That justifies adding `io.flutter` (and analogous library
//! namespaces) to the known-FP family allowlist.
//!
//! # SKIP path
//!
//! Artifacts are NOT committed until the regen pipeline is exercised
//! on a machine with Android SDK + kotlinc. Before then this test
//! SKIPs cleanly with a message pointing at the regen script. Once the
//! artifacts are present, the assertions above are enforced.

mod common;

use std::path::PathBuf;

use common::r8_canonical_marker::{descriptor_to_mapping_key, parse_block_outlined_marker};
use common::r8_mapping_outline::OutlineSet;

const FIXTURE_REL: &str = "tests/fixtures/r8/library_helper_rename/artifacts";
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

/// The container class the Kotlin source defines. After R8 minification
/// this name appears on the LEFT-hand side of the mapping line
/// (`MyLibraryHelpers -> a:` style). Used to slice the mapping into
/// the engineered-FP region we make assertions on.
const HELPER_ORIGINAL_CLASS: &str = "fox.droidsaw.r8fixture.libstub.MyLibraryHelpers";

/// Original-name prefix for the renamed helper namespace. The
/// recogniser-fires-on-FP assertion is scoped to descriptors that
/// mapping-key back to a class whose ORIGINAL name lives under this
/// namespace.
const HELPER_NAMESPACE_PREFIX: &str = "fox.droidsaw.r8fixture.libstub";

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_REL)
        .join(name)
}

#[test]
fn r8_library_helper_rename_fixture_documents_recogniser_fp() {
    let handle = std::thread::Builder::new()
        .name("r8_library_helper_rename_worker".into())
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
            "SKIP: in-tree R8 library-helper-rename fixture artifacts not present at {}. \
             Run tests/fixtures/r8/library_helper_rename/scripts/regen-library-helper-fixture.sh \
             to generate classes.dex + mapping.txt from the engineered Kotlin source. \
             Requires Android SDK build-tools (r8 / d8) + kotlinc.",
            dex_path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
        );
        return;
    }

    // Read the mapping as ground truth.
    let outlines = match OutlineSet::from_file(&mapping_path) {
        Ok(s) => s,
        Err(e) => panic!(
            "in-tree library-helper-rename fixture mapping.txt failed to parse: {e} \
             (path: {}). Re-run the regen script.",
            mapping_path.display(),
        ),
    };

    // Also slurp the mapping as raw text so we can reason about
    // (a) class renames present on MyLibraryHelpers, (b) the absence
    // of `com.android.tools.r8.outline` annotations on the helper slice.
    let mapping_text = std::fs::read_to_string(&mapping_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", mapping_path.display()));

    // (1) The minifier ran. Either the container class line or one
    // of its method lines should mention the original FQN.
    assert!(
        mapping_text.contains(HELPER_ORIGINAL_CLASS),
        "mapping.txt does not mention {HELPER_ORIGINAL_CLASS}; the minifier did not run or the \
         keep rule preserved the original name. Re-check source/proguard-rules.pro: the helper \
         class must be kept with `allowshrinking,allowobfuscation`, NOT with a surface-preserving \
         keep.",
    );

    // (2) NO outline annotations on the helper slice. We scan the
    // raw mapping for any line that mentions BOTH the helper class
    // (or its renamed shadow under the same context) and an outline
    // annotation. Lines that have the outline marker but reference
    // an unrelated kotlin-stdlib class are fine.
    let helper_outline_hits: Vec<&str> = mapping_text
        .lines()
        .filter(|line| {
            line.contains("com.android.tools.r8.outline")
                && line.contains(HELPER_NAMESPACE_PREFIX)
        })
        .collect();
    assert!(
        helper_outline_hits.is_empty(),
        "mapping.txt carries com.android.tools.r8.outline annotations on the engineered \
         MyLibraryHelpers slice — the Kotlin bodies have collapsed into something R8's \
         outliner can extract. Re-tune source/Main.kt so each helper body remains \
         structurally distinct. Hits:\n  {}",
        helper_outline_hits.iter().take(5).copied().collect::<Vec<_>>().join("\n  "),
    );

    // Parse the DEX and walk every class, recording recogniser markers.
    let data = std::fs::read(&dex_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", dex_path.display()));
    let dex = droidsaw_dex::parser::DexFile::parse(&data, None)
        .unwrap_or_else(|e| panic!("parse {}: {e:?}", dex_path.display()));
    let census = droidsaw_dex::r8_inversion::build_trampoline_census(&dex);

    // Marker tuples keyed (mapping-key class, method).
    let mut all_marker_tuples: std::collections::BTreeSet<(String, String)> =
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
            all_marker_tuples.insert((key_class, marker.helper_method.to_string()));
        }
    }

    // Resolve which markers point at the engineered helper slice.
    // After R8 minification the helper class' renamed name is the
    // mapping-key value (e.g. `a` or `b/c`); we recover the ORIGINAL
    // name via OutlineSet's mapping data so we can scope the FP
    // assertion correctly.
    //
    // OutlineSet doesn't directly expose the reverse rename map, but
    // we don't need it: the mapping text already contains
    // `MyLibraryHelpers -> <short>:` lines. Parse those out to learn
    // which short class names belong to the engineered slice.
    let helper_short_names: std::collections::BTreeSet<String> =
        parse_helper_short_names(&mapping_text);
    assert!(
        !helper_short_names.is_empty(),
        "could not extract any rename target for {HELPER_ORIGINAL_CLASS} from mapping.txt. \
         The mapping format may have changed; check parse_helper_short_names() in this test."
    );

    // (3) The recogniser fired at least one marker on a tuple that
    // matches the helper slice (by renamed-class membership).
    let helper_marker_tuples: Vec<(String, String)> = all_marker_tuples
        .iter()
        .filter(|(class, _method)| helper_short_names.contains(class))
        .cloned()
        .collect();

    eprintln!(
        "LIBRARY-HELPER-RENAME RATCHET: {} classes scanned, {} total markers, \
         {} distinct (class, method) tuples, {} tuples on the helper slice \
         ({} short-name(s): {:?})",
        total_classes,
        total_markers,
        all_marker_tuples.len(),
        helper_marker_tuples.len(),
        helper_short_names.len(),
        helper_short_names,
    );

    // The recogniser MUST fire on the helper slice. If it doesn't,
    // either R8's minifier elided the helper class (unlikely with our
    // dense fan-in) or the recogniser's structural predicates I4-I13
    // have drifted and no longer accept the shape. Both are signals.
    assert!(
        !helper_marker_tuples.is_empty(),
        "recogniser fired ZERO markers on the renamed MyLibraryHelpers slice. \
         Either the I4-I13 predicates no longer match this structural shape, or \
         R8 elided the helper class entirely. Total markers across the DEX: {total_markers}.",
    );

    // (4) THE LOAD-BEARING ASSERTION — every helper-slice marker is
    // a FALSE POSITIVE per the mapping. None should resolve to an
    // outline annotation. This is the documented FP family: the
    // recogniser fires structurally, the mapping says "not an
    // outline", and the disagreement is the empirical justification
    // for adding R8-renamed library namespaces (io.flutter, etc.) to
    // the known-FP allowlist.
    let confirmed_outlines_on_slice: Vec<(String, String)> = helper_marker_tuples
        .iter()
        .filter(|(class, method)| outlines.outlined_kind(class, method).is_some())
        .cloned()
        .collect();
    assert!(
        confirmed_outlines_on_slice.is_empty(),
        "the mapping confirms outline annotations on {n} helper-slice marker(s) — that \
         contradicts the fixture's premise (R8 should MINIFY but NOT OUTLINE the helpers). \
         If R8 has started outlining these bodies, the Kotlin source has lost the structural \
         variation that prevents extraction; re-tune source/Main.kt. Confirmed-outline tuples: \
         {confirmed_outlines_on_slice:?}",
        n = confirmed_outlines_on_slice.len(),
    );

    eprintln!(
        "DOCUMENTED FP FAMILY: {} recogniser marker(s) on the renamed MyLibraryHelpers \
         slice, ZERO of which the mapping confirms as outlined. The recogniser cannot \
         distinguish R8-renamed library helper code from R8-synthesized outline helpers \
         by DEX shape alone — the mapping is the disambiguator. This is the empirical \
         anchor for the io.flutter known-FP family entry.",
        helper_marker_tuples.len(),
    );
}

/// Pull every `MyLibraryHelpers -> <short>:` rename target out of the
/// mapping text. Returns the set of RENAMED class names — those are
/// the values the recogniser sees as the helper class' identity after
/// R8 minification.
///
/// Mapping format (R8's pg-map shape, abbreviated):
///   `original.qualified.Name -> short.qualified.Name:`
fn parse_helper_short_names(mapping_text: &str) -> std::collections::BTreeSet<String> {
    let mut out: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let prefix = format!("{HELPER_ORIGINAL_CLASS} -> ");
    for line in mapping_text.lines() {
        let trimmed = line.trim_start();
        let Some(after) = trimmed.strip_prefix(&prefix) else {
            continue;
        };
        let Some(short) = after.strip_suffix(':') else {
            continue;
        };
        let short = short.trim();
        if !short.is_empty() {
            out.insert(short.to_string());
        }
    }
    out
}
