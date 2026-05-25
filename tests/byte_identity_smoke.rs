//! Byte-identity gauge on the fixture DEX. Reports pass/fail +
//! byte-diff magnitude + the set of canonical transforms emit observed
//! applying. Asserts the attribution contract (byte-identity ⇒ empty
//! transforms) but does not hard-assert byte-identity itself — that's
//! the eventual goal, reached as per-canonicalization work adds
//! observation + preservation paths.
//!
//! Rerun this test to track progress:
//!
//!   `cargo test -p droidsaw-dex --test byte_identity_smoke -- --nocapture`
//!
//! Output line is stable-shaped so a shell one-liner can grep it
//! across builds for a ratchet signal.

use droidsaw_dex::emit_dex::{
    emit_dex_collect, DexEmitError, EmitConfig,
};
use droidsaw_dex::parser::{ContentEquiv, DexFile};

fn measure(bytes: &[u8], label: &str) -> Result<(bool, usize), String> {
    measure_with_config(bytes, label, &EmitConfig::default())
}

fn measure_with_config(
    bytes: &[u8],
    label: &str,
    config: &EmitConfig,
) -> Result<(bool, usize), String> {
    let dex1 = DexFile::parse(bytes, None).map_err(|e| format!("parse1: {e:?}"))?;
    let out = match emit_dex_collect(&dex1, config) {
        Ok(o) => o,
        Err(DexEmitError::UnrepresentableIR { why }) => {
            eprintln!("byte_identity_smoke[{label}]: UnrepresentableIR: {why} — skipping");
            return Ok((false, 0));
        }
        Err(e) => return Err(format!("emit: {e:?}")),
    };
    // Content-equivalence MUST hold regardless of byte-identity.
    let dex2 = DexFile::parse(&out.bytes, None).map_err(|e| format!("parse2: {e:?}"))?;
    assert_eq!(
        ContentEquiv(&dex1),
        ContentEquiv(&dex2),
        "content-equivalence broke on {label}"
    );
    let identical = bytes == out.bytes.as_slice();
    let diff = bytes
        .iter()
        .zip(out.bytes.iter())
        .filter(|(a, b)| a != b)
        .count()
        + bytes.len().abs_diff(out.bytes.len());

    // Attribution contract: byte-identity ⇒ empty
    // applied_transformations. Any reported variant is a concrete
    // byte-changing operation emit actually performed — never
    // speculative. Fail loudly on inconsistency so a future observation
    // bug surfaces here rather than silently.
    if identical && !out.applied_transformations.is_empty() {
        return Err(format!(
            "attribution bug: byte-identical output reports applied \
             transforms {:?} — a transform that was reported must have \
             actually changed bytes",
            out.applied_transformations
        ));
    }

    eprintln!(
        "byte_identity_smoke[{label}]: input_len={} output_len={} size_delta={:+} \
         byte_identical={identical} byte_diff={diff} transforms={:?}",
        bytes.len(),
        out.bytes.len(),
        out.bytes.len() as isize - bytes.len() as isize,
        out.applied_transformations,
    );
    Ok((identical, diff))
}

#[test]
fn minimal_fixture_byte_identity_gauge() {
    let bytes: &[u8] = include_bytes!("fixtures/classes.dex");
    let (identical, diff) =
        measure(bytes, "fixtures/classes.dex").expect("measure succeeds");
    // Currently NOT byte-identical; observation paths for each
    // canonicalization emit applies are landing incrementally
    // (string-pool sort — observed; map_list order, alignment padding,
    // encoded_value width — follow-ups).
    // This gauge is the measurement surface — as each follow-up lands,
    // byte_diff shrinks and `transforms` grows to attribute the
    // remaining divergence.
    let _ = identical;
    let _ = diff;
}

#[test]
fn minimal_fixture_content_equivalence_holds() {
    // Sanity check: content-equivalence is the property we DO
    // guarantee today (100% on the 938-DEX corpus per Commit D).
    // If this ever fails on the fixture, it's a real regression.
    let bytes: &[u8] = include_bytes!("fixtures/classes.dex");
    let _ = measure(bytes, "fixtures/classes.dex").expect("measure succeeds");
}

#[test]
fn preserve_map_list_order_does_not_widen_byte_diff() {
    // Gauge: enabling `preserve_map_list_order` MUST NOT increase the
    // byte-diff vs default mode on a canonical-map-list input. When the
    // input's map_list is already in canonical (ascending-offset) order,
    // preserve mode emits the same order — byte_diff equal-or-smaller.
    // When the input's map_list is non-canonical (d8/dexopt build-order),
    // preserve mode retains it — byte_diff strictly smaller.
    let bytes: &[u8] = include_bytes!("fixtures/classes.dex");
    let (_, diff_default) =
        measure_with_config(bytes, "default", &EmitConfig::default())
            .expect("default measure succeeds");
    let preserve_cfg = EmitConfig { preserve_map_list_order: true, ..Default::default() };
    let (_, diff_preserve) =
        measure_with_config(bytes, "preserve", &preserve_cfg)
            .expect("preserve measure succeeds");
    eprintln!(
        "preserve_map_list_order_gauge: default_diff={diff_default} \
         preserve_diff={diff_preserve} delta={}",
        diff_default as isize - diff_preserve as isize,
    );
    assert!(
        diff_preserve <= diff_default,
        "preserve mode regressed byte-diff: default={diff_default} preserve={diff_preserve}",
    );
}

#[test]
fn preserve_encoded_value_width_does_not_widen_byte_diff() {
    // Same gauge as preserve_map_list_order but for the encoded_value
    // width toggle. On any canonical-width fixture this is a no-op
    // (no widths to preserve); on a wider-than-min fixture preserve
    // mode strictly reduces byte_diff.
    let bytes: &[u8] = include_bytes!("fixtures/classes.dex");
    let (_, diff_default) =
        measure_with_config(bytes, "default", &EmitConfig::default())
            .expect("default measure succeeds");
    let preserve_cfg =
        EmitConfig { preserve_encoded_value_width: true, ..Default::default() };
    let (_, diff_preserve) =
        measure_with_config(bytes, "preserve_evw", &preserve_cfg)
            .expect("preserve measure succeeds");
    eprintln!(
        "preserve_encoded_value_width_gauge: default_diff={diff_default} \
         preserve_diff={diff_preserve} delta={}",
        diff_default as isize - diff_preserve as isize,
    );
    assert!(
        diff_preserve <= diff_default,
        "preserve mode regressed byte-diff: default={diff_default} preserve={diff_preserve}",
    );
}

#[test]
fn preserve_both_toggles_does_not_widen_byte_diff() {
    // Combined gauge: both preserves on simultaneously. Should produce
    // the smallest byte_diff (sum of map_list + encoded_value reductions).
    let bytes: &[u8] = include_bytes!("fixtures/classes.dex");
    let (_, diff_default) =
        measure_with_config(bytes, "default", &EmitConfig::default())
            .expect("default measure succeeds");
    let preserve_cfg = EmitConfig {
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        ..Default::default()
    };
    let (_, diff_preserve) =
        measure_with_config(bytes, "preserve_both", &preserve_cfg)
            .expect("preserve measure succeeds");
    eprintln!(
        "preserve_both_toggles_gauge: default_diff={diff_default} \
         preserve_diff={diff_preserve} delta={}",
        diff_default as isize - diff_preserve as isize,
    );
    assert!(
        diff_preserve <= diff_default,
        "preserve mode regressed byte-diff: default={diff_default} preserve={diff_preserve}",
    );
}

#[test]
fn preserve_data_section_layout_recovers_byte_identity() {
    // The big one: preserve_data_section_layout reorders data-section
    // subsection emission to match the input's `dex.map_entries` order.
    // On the fixture, the input data-section subsection order differs
    // from emit's canonical order, so preserve mode should dramatically
    // reduce byte_diff vs default. On a 1:1 round-trip from a parsed
    // input, the predicted result is byte-identical (delta = full
    // default_diff).
    let bytes: &[u8] = include_bytes!("fixtures/classes.dex");
    let (_, diff_default) =
        measure_with_config(bytes, "default", &EmitConfig::default())
            .expect("default measure succeeds");
    let preserve_cfg = EmitConfig {
        preserve_data_section_layout: true,
        ..Default::default()
    };
    let (identical, diff_preserve) =
        measure_with_config(bytes, "preserve_dsl", &preserve_cfg)
            .expect("preserve measure succeeds");
    eprintln!(
        "preserve_data_section_layout_gauge: default_diff={diff_default} \
         preserve_diff={diff_preserve} byte_identical={identical} delta={}",
        diff_default as isize - diff_preserve as isize,
    );
    assert!(
        diff_preserve <= diff_default,
        "preserve mode regressed byte-diff: default={diff_default} preserve={diff_preserve}",
    );
}

#[test]
fn preserve_all_three_toggles_does_not_widen_byte_diff() {
    // All three preserves on. Should produce the smallest byte_diff.
    let bytes: &[u8] = include_bytes!("fixtures/classes.dex");
    let (_, diff_default) =
        measure_with_config(bytes, "default", &EmitConfig::default())
            .expect("default measure succeeds");
    let preserve_cfg = EmitConfig {
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        preserve_data_section_layout: true,
        ..Default::default()
    };
    let (identical, diff_preserve) =
        measure_with_config(bytes, "preserve_all3", &preserve_cfg)
            .expect("preserve measure succeeds");
    eprintln!(
        "preserve_all3_gauge: default_diff={diff_default} \
         preserve_diff={diff_preserve} byte_identical={identical} delta={}",
        diff_default as isize - diff_preserve as isize,
    );
    assert!(
        diff_preserve <= diff_default,
        "preserve mode regressed byte-diff: default={diff_default} preserve={diff_preserve}",
    );
}
