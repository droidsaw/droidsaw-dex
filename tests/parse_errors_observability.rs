//! Parse-error visibility gate: adversarial DEX whose class_data_off
//! points past EOF must parse tolerantly AND surface the skip via
//! `DexFile.parse_errors`. Replaces the prior silent-drop behavior.
//!
//! MVP scope: exercise one of the seven newly-recorded subsections
//! (class_data) end-to-end; the other six (AnnotationDirectory,
//! AnnotationSet, AnnotationSetRefList, AnnotationItem, EncodedArray,
//! CodeItem, DebugInfo) share the identical record-and-continue
//! shape.

use droidsaw_dex::emit_dex::{emit_dex, emit_dex_with_config, DexEmitError, EmitConfig};
use droidsaw_dex::parser::{DexFile, ParseFailureKind};

#[test]
fn well_formed_classes_dex_has_no_parse_errors() {
    // The canonical fixture is clean; parse_errors must be empty on
    // every production-shaped DEX (d8/r8 output).
    let data: &[u8] = include_bytes!("fixtures/classes.dex");
    let dex = DexFile::parse(data, None).expect("parse Minimal fixture");
    assert!(
        dex.parse_errors.is_empty(),
        "well-formed fixture should have zero parse_errors; got {:?}",
        dex.parse_errors
    );
}

#[test]
fn class_data_off_past_eof_records_skip() {
    // Scan the Minimal fixture for a class_def with `class_data_off != 0`,
    // then corrupt the offset field in-place to point past EOF. The
    // parser should tolerate the skip AND push a `ClassData` entry
    // into `parse_errors`.
    let base: Vec<u8> = include_bytes!("fixtures/classes.dex").to_vec();
    let pristine = DexFile::parse(&base, None).expect("parse pristine");
    // Pick the first non-zero class_data_off.
    let (class_idx, original_off) = pristine
        .class_defs
        .iter()
        .enumerate()
        .find(|(_, cd)| cd.class_data_off != 0)
        .map(|(i, cd)| (i, cd.class_data_off))
        .expect("fixture has a populated class_data_off");

    // `class_def_item` stride = 32 bytes (spec §"class_def_item"),
    // `class_data_off` at byte offset 24 within the record.
    let class_def_abs = pristine.header.class_defs_off as usize + class_idx * 32;
    let cdo_abs = class_def_abs + 24;
    let mut corrupt = base.clone();
    // Point past the end of the buffer — guaranteed malformed.
    let past_eof = (base.len() as u32).saturating_add(0x1000);
    corrupt[cdo_abs..cdo_abs + 4].copy_from_slice(&past_eof.to_le_bytes());
    // Recompute + patch the Adler-32 checksum (bytes 8..12, covers
    // [12..file_size]) so `verify_checksum` on re-parse accepts the
    // mutation.
    let new_checksum = adler2::adler32_slice(&corrupt[12..]);
    corrupt[8..12].copy_from_slice(&new_checksum.to_le_bytes());

    let dex = DexFile::parse(&corrupt, None).expect(
        "parser should tolerate malformed class_data_off (silent-skip → record)",
    );
    assert!(
        !dex.parse_errors.is_empty(),
        "adversarial class_data_off should record a parse failure; got empty parse_errors"
    );
    let has_class_data_skip = dex.parse_errors.iter().any(|pf| {
        matches!(pf.kind, ParseFailureKind::ClassData)
            && (pf.offset == past_eof || pf.offset == original_off)
    });
    assert!(
        has_class_data_skip,
        "expected ClassData skip at offset {past_eof:#x}; got {:?}",
        dex.parse_errors
    );

    // Emit-side gate: `emit_dex` (strict default) must REFUSE to
    // round-trip a DEX whose `parse_errors` is populated. Otherwise the
    // attacker hides bytes in malformed subsections, parse drops them,
    // and emit produces a clean corpus-ready DEX that lacks the hidden
    // content — an evasion primitive.
    match emit_dex(&dex) {
        Err(DexEmitError::PartialIR {
            count,
            first_kind,
            ..
        }) => {
            assert!(count >= 1);
            assert_eq!(first_kind, ParseFailureKind::ClassData);
        }
        Ok(_) => panic!("strict emit_dex accepted partial IR — evasion-primitive gate broken"),
        Err(other) => panic!("expected PartialIR, got {other:?}"),
    }

    // Opt-in path: `emit_dex_with_config(permit_partial_ir: true)`
    // bypasses the PartialIR gate. The underlying IR still has a
    // dangling class_data reference on this adversarial input, so
    // emit may subsequently fail with `UnrepresentableIR` from a
    // downstream validation — but it MUST NOT return PartialIR
    // (that's the gate the flag turns off). Asserting the
    // narrow-negative: the error class changes from PartialIR to
    // something else.
    let permissive = EmitConfig {
        permit_partial_ir: true,
        preserve_map_list_order: false,
        preserve_encoded_value_width: false,
        preserve_data_section_layout: false,
        preserve_input_checksums: false,
    };
    match emit_dex_with_config(&dex, &permissive) {
        Err(DexEmitError::PartialIR { .. }) => {
            panic!("permit_partial_ir=true should NOT hit the PartialIR gate")
        }
        Ok(_) | Err(_) => {
            // Any other outcome is acceptable — the gate is what's
            // under test here, not emit's downstream validation.
        }
    }
}

#[test]
fn emit_dex_strict_accepts_clean_parse() {
    // Sanity check: the strict default has zero effect on
    // well-formed inputs.
    let data: &[u8] = include_bytes!("fixtures/classes.dex");
    let dex = DexFile::parse(data, None).expect("parse");
    assert!(dex.parse_errors.is_empty());
    let _ = emit_dex(&dex).expect("strict emit on clean input");
}

/// Helper: corrupt a single `class_def_item` offset field to point past
/// EOF and re-patch the Adler-32 checksum so re-parse accepts the body.
/// `field_byte_off` is the in-record offset of the field
/// (interfaces_off=12, annotations_off=20, class_data_off=24,
/// static_values_off=28; see DEX spec §"class_def_item").
fn corrupt_class_def_off(base: &[u8], class_idx: usize, field_byte_off: usize) -> (Vec<u8>, u32) {
    let pristine = DexFile::parse(base, None).expect("parse pristine");
    let class_def_abs = pristine.header.class_defs_off as usize + class_idx * 32;
    let off_abs = class_def_abs + field_byte_off;
    let mut corrupt = base.to_vec();
    let past_eof = (base.len() as u32).saturating_add(0x1000);
    corrupt[off_abs..off_abs + 4].copy_from_slice(&past_eof.to_le_bytes());
    let new_checksum = adler2::adler32_slice(&corrupt[12..]);
    corrupt[8..12].copy_from_slice(&new_checksum.to_le_bytes());
    (corrupt, past_eof)
}

/// Symmetry gauge: the four class_def offset fields each route through
/// the tolerant-parse-and-record discipline. The original
/// `class_data_off_past_eof_records_skip` test exercises one; these
/// three lock the contract for the other three (`interfaces_off`,
/// `annotations_off`, `static_values_off`). Same shape: parse Ok,
/// `parse_errors` populated with the expected kind, strict
/// `emit_dex` refuses with `PartialIR`.

#[test]
fn interfaces_off_past_eof_records_skip() {
    let base: &[u8] = include_bytes!("fixtures/classes.dex");
    // Patch class_def[0] unconditionally. The corruption sets the
    // offset to a non-zero past-EOF value, so the parser will try to
    // read it regardless of its pristine value (zero would have
    // short-circuited before any read).
    let (corrupt, past_eof) = corrupt_class_def_off(base, 0, 12);
    let dex = DexFile::parse(&corrupt, None).expect(
        "parser must tolerate malformed interfaces_off (silent-skip → record)",
    );
    assert!(
        dex.parse_errors
            .iter()
            .any(|pf| matches!(pf.kind, ParseFailureKind::Interfaces) && pf.offset == past_eof),
        "expected Interfaces skip at {past_eof:#x}; got {:?}",
        dex.parse_errors
    );
    match emit_dex(&dex) {
        Err(DexEmitError::PartialIR { first_kind, .. }) => {
            assert_eq!(first_kind, ParseFailureKind::Interfaces);
        }
        Ok(_) => panic!("strict emit_dex accepted partial IR — evasion-primitive gate broken"),
        Err(other) => panic!("expected PartialIR, got {other:?}"),
    }
}

#[test]
fn annotations_off_past_eof_records_skip() {
    let base: &[u8] = include_bytes!("fixtures/classes.dex");
    let (corrupt, past_eof) = corrupt_class_def_off(base, 0, 20);
    let dex = DexFile::parse(&corrupt, None).expect(
        "parser must tolerate malformed annotations_off (silent-skip → record)",
    );
    assert!(
        dex.parse_errors.iter().any(|pf| matches!(
            pf.kind,
            ParseFailureKind::AnnotationDirectory
        ) && pf.offset == past_eof),
        "expected AnnotationDirectory skip at {past_eof:#x}; got {:?}",
        dex.parse_errors
    );
    match emit_dex(&dex) {
        Err(DexEmitError::PartialIR { first_kind, .. }) => {
            assert_eq!(first_kind, ParseFailureKind::AnnotationDirectory);
        }
        Ok(_) => panic!("strict emit_dex accepted partial IR — evasion-primitive gate broken"),
        Err(other) => panic!("expected PartialIR, got {other:?}"),
    }
}

#[test]
fn static_values_off_past_eof_records_skip() {
    let base: &[u8] = include_bytes!("fixtures/classes.dex");
    let (corrupt, past_eof) = corrupt_class_def_off(base, 0, 28);
    let dex = DexFile::parse(&corrupt, None).expect(
        "parser must tolerate malformed static_values_off (silent-skip → record)",
    );
    assert!(
        dex.parse_errors
            .iter()
            .any(|pf| matches!(pf.kind, ParseFailureKind::EncodedArray) && pf.offset == past_eof),
        "expected EncodedArray skip at {past_eof:#x}; got {:?}",
        dex.parse_errors
    );
    match emit_dex(&dex) {
        Err(DexEmitError::PartialIR { first_kind, .. }) => {
            assert_eq!(first_kind, ParseFailureKind::EncodedArray);
        }
        Ok(_) => panic!("strict emit_dex accepted partial IR — evasion-primitive gate broken"),
        Err(other) => panic!("expected PartialIR, got {other:?}"),
    }
}
