//! Adversarial PoC ratchet — family-prefix masquerade (negative control).
//!
//! Demonstrates THREE coupled defense layers the family-prefix filter
//! relies on against a naive masquerade attempt — and surfaces a
//! finding the original threat model on `KNOWN_FP_FAMILY` understated:
//! the recogniser ALSO requires `ACC_SYNTHETIC` (0x1000) on the helper
//! class (`r8_inversion.rs:796`). kotlinc-emitted developer classes
//! don't carry that flag, so a "name a class in androidx.* and give it
//! outline-shape" attempt does NOT trigger the recogniser at all.
//!
//! The fixture is a NEGATIVE CONTROL: source defines a Kotlin
//! `androidx.adversarial.poc.Backdoor` with 25 callers satisfying
//! I4–I13. The recogniser does NOT fire because the class lacks
//! ACC_SYNTHETIC. This is good news — the defense is stronger than
//! the bare family-prefix filter would imply.
//!
//! A complete masquerade requires the attacker to also forge
//! ACC_SYNTHETIC on the class (via smali, raw DEX emit, or
//! post-process bit-flip). The fixture's source is the pre-image of
//! that attack; the ACC_SYNTHETIC forgery step is left as a future
//! tooling extension (smali requires additional CI dependencies).
//!
//! Test asserts NEGATIVE behaviour: the recogniser must NOT fire on
//! the adversarial slice. A future change that drops or weakens the
//! ACC_SYNTHETIC gate would cause this test to start firing markers
//! and fail — surfacing the defense regression immediately.
//!
//! # SKIP path
//!
//! Artifacts are NOT checked in until the regen pipeline runs on a
//! machine with Android SDK + kotlinc. Before that, the test SKIPs
//! cleanly.
//!
//! # Assertions (negative control: defense layers hold)
//!
//! 1. Mapping LHS preserves `androidx.adversarial.poc.*` class names
//!    (proves the masquerade FQCN premise; an attacker can put a
//!    class in androidx.* — DEX has no namespace enforcement).
//! 2. `matches_androidx_family()` returns true on those classes
//!    (confirms the family filter WOULD suppress them if the
//!    recogniser fired).
//! 3. `classify_synthetic_kind()` returns `Unknown` on them
//!    (confirms zero structural attestation in obfuscated form).
//! 4. Mapping carries no outline annotation on the slice (these are
//!    NOT real R8 outlines).
//! 5. **The recogniser fires ZERO markers on the slice.** This is
//!    the defense layer the original threat model understated:
//!    `ACC_SYNTHETIC` gates the recogniser's per-class admission.
//!    A future regression that drops the gate fails this assertion.
//! 6. The literal `"PWNED:"` string survives in the DEX string table.
//!    Confirms the malicious code is still inspectable (the family
//!    filter, even if it DID fire, would be lens-blinding — not
//!    code-hiding).

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use common::r8_canonical_marker::{descriptor_to_mapping_key, parse_block_outlined_marker};
use common::r8_mapping_outline::{classify_synthetic_kind, OutlineSet, SyntheticKind};

const FIXTURE_REL: &str = "tests/fixtures/r8/family_prefix_masquerade/artifacts";
const SMOKE_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Masquerade namespace. Kept in sync with the Kotlin `package`
/// declaration in `source/Main.kt`. Spoofs `androidx.*` lexically;
/// no real AndroidX provenance — `adversarial.poc` is a placeholder.
const MASQUERADE_PREFIX: &str = "androidx.adversarial.poc.";

/// Mirror of `tests/r8_fdroid_apk_sweep.rs::is_in_known_fp_family`,
/// restricted to the `androidx` entry the masquerade targets. Kept
/// inline so this fixture can ratchet against the family-suppression
/// behaviour without coupling to harness internals. If the harness
/// changes the family-match semantics (currently `class == entry ||
/// class.starts_with(entry + ".")`), update here.
fn matches_androidx_family(key_class: &str) -> bool {
    key_class == "androidx" || key_class.starts_with("androidx.")
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_REL)
        .join(name)
}

#[test]
fn r8_family_prefix_masquerade_poc() {
    let handle = std::thread::Builder::new()
        .name("r8_family_prefix_masquerade_worker".into())
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
            "SKIP: family-prefix-masquerade fixture artifacts not present at {}. \
             Run tests/fixtures/r8/family_prefix_masquerade/scripts/regen-family-prefix-masquerade-fixture.sh \
             on a machine with kotlinc + R8.",
            dex_path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
        );
        return;
    }

    let mapping_text = std::fs::read_to_string(&mapping_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", mapping_path.display()));

    // Collect class renames whose ORIGINAL class lives under
    // androidx.adversarial.poc.* — gives us the obfuscated class
    // names that should appear in the DEX.
    let renames = collect_renames_under_prefix(&mapping_text, MASQUERADE_PREFIX);
    assert!(
        !renames.is_empty(),
        "mapping.txt has zero `{MASQUERADE_PREFIX}*` class rename records. \
         The masquerade premise requires the kotlinc-produced classes to \
         actually appear in the mapping LHS — if missing, the package \
         declaration in source/Main.kt may have drifted, or R8 tree-shook \
         the entire slice."
    );

    // Obfuscated class names that R8 emitted for the masquerade slice.
    // Marker classes from the recogniser will use these (R8 may have
    // renamed androidx.adversarial.poc.Backdoor to a short name within
    // the same namespace, e.g. `androidx.adversarial.poc.a`).
    let obfuscated_classes: BTreeSet<String> =
        renames.iter().map(|(_, obf)| obf.clone()).collect();

    // (4) Mapping has no outline annotations on the adversarial slice.
    let outlines = OutlineSet::from_file(&mapping_path).unwrap_or_else(|e| {
        panic!("parse OutlineSet from {}: {e}", mapping_path.display())
    });
    let leaked_outlines: Vec<(String, String)> = outlines
        .outlined_methods()
        .filter(|(c, _)| obfuscated_classes.contains(*c))
        .map(|(c, m)| (c.to_string(), m.to_string()))
        .collect();
    assert!(
        leaked_outlines.is_empty(),
        "{} com.android.tools.r8.outline annotation(s) on the adversarial \
         slice in mapping.txt. The masquerade demonstration requires R8 to \
         have NOT outlined the helper (otherwise the marker is a true \
         positive, not a masquerade). Re-tune source/Main.kt body or \
         proguard-rules.pro. First offenders:\n  {}",
        leaked_outlines.len(),
        leaked_outlines
            .iter()
            .take(5)
            .map(|(c, m)| format!("{c}.{m}"))
            .collect::<Vec<_>>()
            .join("\n  "),
    );

    // Run the recogniser over the DEX.
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

    // (1) Recogniser fired on the adversarial slice (post-rename
    // obfuscated class within androidx.adversarial.poc.*).
    let adversarial_markers: Vec<(String, String)> = marker_tuples
        .iter()
        .filter(|(c, _)| obfuscated_classes.contains(c))
        .cloned()
        .collect();

    eprintln!(
        "FAMILY-PREFIX MASQUERADE (negative control): {} classes total, {} markers fired, {} distinct tuples",
        total_classes,
        total_markers,
        marker_tuples.len(),
    );
    eprintln!(
        "  mapping LHS adversarial classes: {}",
        renames.len(),
    );
    eprintln!(
        "  recogniser markers on adversarial slice: {} (expected 0 -- ACC_SYNTHETIC gate)",
        adversarial_markers.len(),
    );

    // (2) Even though the recogniser doesn't fire (per ACC_SYNTHETIC
    // gate), the prefix-match semantics on the obfuscated classes
    // still hold. If a future tooling extension forges ACC_SYNTHETIC
    // and the recogniser does start firing, this confirms the family
    // filter would (incorrectly) suppress the resulting markers.
    for obf in &obfuscated_classes {
        assert!(
            matches_androidx_family(obf),
            "adversarial obfuscated class {obf} did NOT match the androidx \
             family prefix check — masquerade FQCN premise lost. Either \
             R8 moved the class out of androidx.* (check `--no-minification` \
             flag in regen script) or the family-match semantics drifted."
        );
    }

    // (3) Structural attestation: every obfuscated adversarial class
    // returns SyntheticKind::Unknown. No EnumUnboxingLocalUtility
    // suffix, no $$ExternalSynthetic infix — the attacker didn't
    // bother to forge them and kotlinc doesn't emit them naturally
    // for ordinary code.
    for obf in &obfuscated_classes {
        let kind = classify_synthetic_kind(obf);
        assert_eq!(
            kind,
            SyntheticKind::Unknown,
            "adversarial obfuscated class {obf} unexpectedly classified as \
             {:?} — the source/Main.kt shape would need to move away from \
             anything that triggers a structural pattern.",
            kind,
        );
    }

    // (5) KEY NEGATIVE-CONTROL ASSERTION: the recogniser must NOT
    // fire on the adversarial slice. The ACC_SYNTHETIC gate
    // (r8_inversion.rs:796) rejects kotlinc-emitted classes; the
    // masquerade attempt is blocked by this layer of defence.
    // A future regression that drops or weakens that gate causes
    // markers to appear here and this assertion fails — surfacing
    // the defence regression at test time.
    assert!(
        adversarial_markers.is_empty(),
        "recogniser fired {} marker(s) on the adversarial slice — \
         this means the ACC_SYNTHETIC class-flag gate that \
         r8_inversion.rs:796 SHOULD reject kotlinc-emitted classes \
         on no longer holds. Either the gate was removed/weakened, \
         or kotlinc / R8 now sets ACC_SYNTHETIC on something it \
         previously did not. The defence-layer regression must be \
         understood before this test passes again. First markers:\n  {}",
        adversarial_markers.len(),
        adversarial_markers
            .iter()
            .take(5)
            .map(|(c, m)| format!("{c}.{m}"))
            .collect::<Vec<_>>()
            .join("\n  "),
    );

    // (5) Code-is-present sanity: the literal "PWNED:" string from
    // Backdoor.execute survives in the DEX string table. Proves the
    // masquerade is LENS-BLINDING (the sweep summary is wrong) and
    // NOT CODE-HIDING (the malware is still inspectable by strings,
    // xrefs, decompiler output, etc.).
    let pwned_present = dex.strings.iter().any(|s| s.as_str_lossy().contains("PWNED:"));
    assert!(
        pwned_present,
        "literal \"PWNED:\" string from source/Main.kt's Backdoor.execute \
         is NOT present in the DEX string table. Either R8 inlined the \
         constant away or the source drifted; the masquerade demo loses \
         the 'code is still there, only the marker count lies' anchor."
    );

    eprintln!(
        "  PWNED string survival: confirmed (masquerade is lens-blinding, not code-hiding)"
    );
    eprintln!(
        "  family-suppression: {} adversarial marker(s) would be counted in `androidx` bucket",
        adversarial_markers.len(),
    );
    eprintln!(
        "  structural attestation: 0 of {} adversarial markers carry a structural pattern",
        adversarial_markers.len(),
    );
}

/// Scan mapping.txt class-rename records whose ORIGINAL class name
/// starts with `prefix`. Returns (original, obfuscated) tuples.
fn collect_renames_under_prefix(mapping_text: &str, prefix: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in mapping_text.lines() {
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
        if !orig.starts_with(prefix) {
            continue;
        }
        out.push((orig.to_string(), obf.to_string()));
    }
    out
}
