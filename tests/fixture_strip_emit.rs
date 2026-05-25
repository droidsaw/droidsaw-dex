//! Fixture-strip emit experiment.
//!
//! The real classes.dex fixture has annotations and static values —
//! subsections `emit_dex` doesn't yet support. This test zeros those
//! references (a destructive-but-legal DEX mutation; equivalent to
//! `r8 --strip-annotations --no-static-values`) and runs the full
//! emit pipeline on the stripped IR.
//!
//! Goals:
//!   - Surface bugs in the non-annotation machinery on real-world DEX
//!     before we extend to annotations (which is the last gate).
//!   - Measure: does a realistic multi-class, multi-method DEX
//!     round-trip the surface we claim to support?
//!
//! If this test fails it's a signal that something in the pipeline is
//! broken on real input despite the synthetic unit tests passing.

use std::collections::BTreeMap;

use droidsaw_dex::emit_dex::emit_dex;
use droidsaw_dex::parser::DexFile;

const FIXTURE: &[u8] = include_bytes!("fixtures/classes.dex");

/// Strip annotation + static-value references in place, producing a
/// DexFile shape that emit_dex's current feature set can handle.
/// Must clear BOTH the class_defs fields AND the four annotation sub-
/// maps — leaving sub-maps populated creates orphan data (emit writes
/// it, re-parse discards it, idempotence breaks).
fn strip_for_current_emit_scope(dex: &mut DexFile) {
    for c in &mut dex.class_defs {
        c.annotations_off = 0;
        c.static_values_off = 0;
    }
    dex.annotations = BTreeMap::new();
    dex.annotation_sets = BTreeMap::new();
    dex.annotation_set_ref_lists = BTreeMap::new();
    dex.annotation_items = BTreeMap::new();
}

#[test]
fn fixture_round_trips_through_emit_after_stripping_annotations() {
    let mut dex1 = DexFile::parse(FIXTURE, None).expect("fixture parses");
    let original_class_count = dex1.class_defs.len();
    let original_method_count = dex1.methods.len();
    let original_string_count = dex1.strings.len();
    let original_code_count = dex1.code_items.len();
    let original_class_data_count = dex1.class_datas.len();
    let original_type_list_count = dex1.type_lists.len();

    strip_for_current_emit_scope(&mut dex1);

    let emitted = emit_dex(&dex1).expect("emit_dex succeeds on stripped IR");
    let dex2 = DexFile::parse(&emitted, None).expect("re-parse of emitted bytes");

    // Pool counts preserved.
    assert_eq!(
        dex2.class_defs.len(),
        original_class_count,
        "class count preserved"
    );
    assert_eq!(
        dex2.methods.len(),
        original_method_count,
        "method count preserved"
    );
    assert_eq!(
        dex2.strings.len(),
        original_string_count,
        "string count preserved"
    );
    assert_eq!(
        dex2.code_items.len(),
        original_code_count,
        "code_item count preserved"
    );
    assert_eq!(
        dex2.class_datas.len(),
        original_class_data_count,
        "class_data count preserved"
    );
    assert_eq!(
        dex2.type_lists.len(),
        original_type_list_count,
        "type_list count preserved"
    );

    // String content preserved as a set (ordering may differ — emit
    // canonicalizes via NonDecreasing::from_sorted, input may not be).
    let s1: std::collections::BTreeSet<_> = dex1.strings.iter().collect();
    let s2: std::collections::BTreeSet<_> = dex2.strings.iter().collect();
    assert_eq!(s1, s2, "string pool content preserved as set");

    // Checksum of the emitted file must validate per the parser's
    // Adler-32 oracle (already verified in-parse since parse() calls
    // verify_checksum). If we got here, that passed.

    // debug_info_off is now set to point into the emitter-owned
    // debug_info_item region of the output (see
    // `emit_debug_info_section`). Security invariant preserved by
    // construction: emitted bytes ARE the original bytes, so the
    // pointer doesn't land on attacker-crafted foreign content.
    // Verify each non-zero debug_info_off lies inside the emitted
    // file's data section (i.e., is a valid offset, not a stale
    // input-file pointer).
    for ci in dex2.code_items.values() {
        if ci.debug_info_off == 0 {
            continue;
        }
        assert!(
            (ci.debug_info_off as usize) < emitted.len(),
            "debug_info_off {} past emitted file size {}",
            ci.debug_info_off,
            emitted.len()
        );
        assert!(
            ci.debug_info_off >= dex2.header.data_off,
            "debug_info_off {} precedes data_off {} — not in data section",
            ci.debug_info_off,
            dex2.header.data_off
        );
    }
}

#[test]
fn fixture_full_round_trips_without_stripping() {
    // No stripping — the real multi-class multi-method multi-annotation
    // fixture, parsed through emit_dex, re-parsed. All data sections
    // are now supported (the last NotImplemented gate for
    // static_values_off landed). This is the capstone: the proptest
    // wired ahead of emit has finally activated.
    let dex1 = DexFile::parse(FIXTURE, None).expect("parse");
    let emitted = emit_dex(&dex1).expect("full emit");
    let dex2 = DexFile::parse(&emitted, None).expect("reparse");

    // Content equivalence via the quotient newtype — semantic
    // round-trip gate defined in parser.rs::ContentEquiv.
    assert_eq!(
        droidsaw_dex::parser::ContentEquiv(&dex1),
        droidsaw_dex::parser::ContentEquiv(&dex2),
        "full fixture round-trip content equivalence"
    );
}

#[test]
fn fixture_with_annotations_kept_round_trips() {
    // Strip ONLY static_values (the last NotImplemented gate). Keep
    // annotations. This activates the full annotation subsection
    // pipeline on the real fixture.
    let mut dex1 = DexFile::parse(FIXTURE, None).expect("parse");
    let original_annotation_count = dex1.annotations.len();
    let original_annotation_item_count = dex1.annotation_items.len();

    // Only strip static_values.
    for c in &mut dex1.class_defs {
        c.static_values_off = 0;
    }

    let emitted = emit_dex(&dex1).expect("emit with annotations");
    let dex2 = DexFile::parse(&emitted, None).expect("reparse");

    // Annotation subsection counts survive.

    assert_eq!(
        dex2.annotations.len(),
        original_annotation_count,
        "annotation_directory count preserved"
    );
    assert_eq!(
        dex2.annotation_items.len(),
        original_annotation_item_count,
        "annotation_item count preserved"
    );

    // For each class that had annotations in the input DEX, the
    // re-parsed class must also have a (non-zero) annotations_off
    // pointing into the new layout.
    let original_annotated_classes: Vec<_> = DexFile::parse(FIXTURE, None)
        .unwrap()
        .class_defs
        .iter()
        .map(|c| c.annotations_off != 0)
        .collect();
    for (i, c) in dex2.class_defs.iter().enumerate() {
        if original_annotated_classes[i] {
            assert_ne!(
                c.annotations_off, 0,
                "class {i} had annotations pre-strip; should still have post-round-trip"
            );
        }
    }
}

#[test]
fn fixture_emit_output_survives_second_round_trip() {
    // Idempotence check: emit(parse(emit(parse(x)))) agrees with
    // emit(parse(x)). If the pipeline has a non-fixed-point bug
    // (rare but subtle), this catches it.
    let mut dex1 = DexFile::parse(FIXTURE, None).expect("parse");
    strip_for_current_emit_scope(&mut dex1);
    let emitted_once = emit_dex(&dex1).expect("emit #1");

    let dex2 = DexFile::parse(&emitted_once, None).expect("re-parse #1");
    let emitted_twice = emit_dex(&dex2).expect("emit #2");

    assert_eq!(
        emitted_once, emitted_twice,
        "emit is a fixed point after one round-trip"
    );
}
