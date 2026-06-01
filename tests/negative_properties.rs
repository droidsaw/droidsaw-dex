//! Negative properties for `DexFile::parse`: a malformed header field must
//! surface its typed outcome, never `Ok` of a silently-wrong IR. Each
//! generator starts from a real, well-formed DEX (the in-tree `classes.dex`
//! d8 fixture), corrupts exactly one header field, and reseals the Adler-32
//! so the corruption — not a checksum mismatch — is what the parser reacts to.
//!
//! Three distinct invalid-input contracts are exercised, matching what the
//! parser actually guarantees per field (not a blanket "invalid → Err"):
//!   - **Hard-Err**: an out-of-bounds primary-section offset (`string_ids_off`)
//!     aborts the parse with a typed `DexError`.
//!   - **Record-and-continue**: an out-of-bounds `map_off` is tolerated —
//!     parse returns `Ok`, recording a `ParseFailureKind::MapList` in
//!     `parse_errors` so consumers can detect the dropped map_list.
//!   - **Aliased-offset detection (RED, ignored)**: overlapping id-section
//!     offsets currently parse `Ok` with no signal; the asserted invariant is
//!     that the overlap is surfaced, so the test flips green when the
//!     offset cross-check lands.

use droidsaw_dex::error::DexError;
use droidsaw_dex::parser::ParseFailureKind;
use droidsaw_dex::DexFile;
use proptest::prelude::*;

/// In-tree minimal real DEX (1236 bytes, produced by d8).
const DEX: &[u8] = include_bytes!("fixtures/classes.dex");

// DEX header field byte offsets (DEX spec §"header_item").
const OFF_CHECKSUM: usize = 8;
const OFF_FILE_SIZE: usize = 32;
const OFF_MAP: usize = 52;
const OFF_STRING_IDS_OFF: usize = 60;
const OFF_TYPE_IDS_OFF: usize = 68;
const OFF_FIELD_IDS_OFF: usize = 84;
const OFF_METHOD_IDS_OFF: usize = 92;
const MAP_TYPE_METHOD_ID_ITEM: u16 = 0x0005;

fn get_u32(buf: &[u8], off: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&buf[off..off + 4]);
    u32::from_le_bytes(b)
}

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// Recompute the Adler-32 over `[12..file_size]` and write it to the checksum
/// field, so a mutated header parses past `verify_checksum`.
fn reseal(buf: &mut [u8]) {
    let file_size = get_u32(buf, OFF_FILE_SIZE) as usize;
    let checksum = adler2::adler32_slice(&buf[12..file_size]);
    put_u32(buf, OFF_CHECKSUM, checksum);
}

/// Patch the `offset` field of the `map_list` entry for `type_code`, leaving
/// the header's own offset field untouched. The map_list is at `header.map_off`
/// as `[u32 count][map_item × count]`, each `map_item` 12 bytes
/// `{u16 type, u16 unused, u32 size, u32 offset}` (offset at item+8). Returns
/// true if an entry was found and patched.
fn patch_map_entry_offset(buf: &mut [u8], type_code: u16, new_off: u32) -> bool {
    let map_off = get_u32(buf, OFF_MAP) as usize;
    let count = get_u32(buf, map_off) as usize;
    for i in 0..count {
        let item = map_off + 4 + i * 12;
        let tc = u16::from_le_bytes([buf[item], buf[item + 1]]);
        if tc == type_code {
            put_u32(buf, item + 8, new_off);
            return true;
        }
    }
    false
}

proptest! {
    /// `string_ids_off` pointed past EOF is a primary-section offset the parser
    /// hard-rejects: the first `pread` of a `string_id_item` lands out of
    /// bounds → typed `DexError::ScrollRead`. (The `bound_count` gate only
    /// bounds the *count* against the file length, not the base offset, so the
    /// failure surfaces at the per-entry read.)
    #[test]
    fn string_ids_off_past_eof_is_hard_err(delta in 0u32..=8_000_000) {
        let mut m = DEX.to_vec();
        let file_size = get_u32(&m, OFF_FILE_SIZE);
        put_u32(&mut m, OFF_STRING_IDS_OFF, file_size.saturating_add(delta));
        reseal(&mut m);
        let r = DexFile::parse(&m, None);
        prop_assert!(
            matches!(r, Err(DexError::ScrollRead { .. })),
            "string_ids_off past EOF must be ScrollRead, got {r:?}"
        );
    }

    /// `map_off` pointed past EOF is a *secondary* structure: the parser does
    /// not abort, it records the dropped map_list as
    /// `ParseFailureKind::MapList` and returns `Ok` (tolerant-parse contract).
    /// Asserting `Err` here would mis-frame the documented record-and-continue
    /// behavior as a failure.
    #[test]
    fn map_off_past_eof_is_record_and_continue(delta in 0u32..=8_000_000) {
        let mut m = DEX.to_vec();
        let file_size = get_u32(&m, OFF_FILE_SIZE);
        put_u32(&mut m, OFF_MAP, file_size.saturating_add(delta));
        reseal(&mut m);
        let dex = match DexFile::parse(&m, None) {
            Ok(d) => d,
            Err(e) => return Err(TestCaseError::fail(format!(
                "map_off past EOF must parse Ok (record-and-continue), got {e:?}"
            ))),
        };
        prop_assert!(
            dex.parse_errors
                .iter()
                .any(|p| p.kind == ParseFailureKind::MapList),
            "expected a MapList parse_error, got {:?}",
            dex.parse_errors
        );
    }
}

/// Aliased id-section offsets are hard-rejected at parse time.
///
/// `field_id_item` and `method_id_item` share an 8-byte on-disk shape, so
/// repointing `method_ids_off` at the `field_ids` region makes the same bytes
/// decode as both. With `method_ids_size` (8) unchanged, the method_ids range
/// `[field_off, field_off + 8*8)` overlaps the field_ids range
/// `[field_off, field_off + 1*8)`, so the parse-time pairwise check rejects it
/// as `DexError::SectionOverlap` before any id-section is loaded — no `Ok` of a
/// wrong method table can escape.
#[test]
fn aliased_method_ids_offset_is_surfaced() {
    let base = DexFile::parse(DEX, None).expect("baseline fixture parses");
    let field_off = base.header.field_ids_off;

    let mut m = DEX.to_vec();
    put_u32(&mut m, OFF_METHOD_IDS_OFF, field_off); // method_ids_off := field_ids_off
    reseal(&mut m);

    let r = DexFile::parse(&m, None);
    assert!(
        matches!(r, Err(DexError::SectionOverlap { .. })),
        "aliased method_ids_off must hard-reject as SectionOverlap, got {r:?}"
    );
}

/// The overlap check is map-independent: zeroing `map_off` (which the
/// header/map cross-check needs to fire) does NOT let the aliased layout
/// through. This is the non-bypassable floor — `DexError::SectionOverlap` still
/// fires, and the error records `map_present=false`.
#[test]
fn aliased_offset_with_no_map_list_still_rejects() {
    let base = DexFile::parse(DEX, None).expect("baseline fixture parses");
    let field_off = base.header.field_ids_off;

    let mut m = DEX.to_vec();
    put_u32(&mut m, OFF_METHOD_IDS_OFF, field_off);
    put_u32(&mut m, OFF_MAP, 0); // no map_list to cross-check against
    reseal(&mut m);

    let r = DexFile::parse(&m, None);
    assert!(
        matches!(r, Err(DexError::SectionOverlap { map_present: false, .. })),
        "overlap with map_off=0 must still hard-reject (map-independent), got {r:?}"
    );
}

/// The check is general, not field/method-specific: overlapping `type_ids`
/// onto `string_ids` is rejected the same way.
#[test]
fn aliased_type_ids_onto_string_ids_rejects() {
    let base = DexFile::parse(DEX, None).expect("baseline fixture parses");
    let string_off = base.header.string_ids_off;

    let mut m = DEX.to_vec();
    put_u32(&mut m, OFF_TYPE_IDS_OFF, string_off); // type_ids_off := string_ids_off
    reseal(&mut m);

    let r = DexFile::parse(&m, None);
    assert!(
        matches!(r, Err(DexError::SectionOverlap { .. })),
        "type_ids aliased onto string_ids must hard-reject, got {r:?}"
    );
}

/// A *moved* (but non-overlapping) section offset — the header still points at
/// the real section so the parse succeeds, but the map_list records a different
/// offset — surfaces as a High `DEX_HEADER_MAP_OFFSET_DISAGREEMENT` Finding
/// (the recoverable, best-effort-parse case that complements the hard-reject).
#[test]
fn map_offset_disagreement_emits_high_finding() {
    use droidsaw_dex::diag::{
        collect_header_map_findings, FINDING_ID_DEX_HEADER_MAP_OFFSET_DISAGREEMENT,
    };
    use droidsaw_common::finding::Severity;

    let real_method_off = get_u32(DEX, OFF_METHOD_IDS_OFF);
    let mut m = DEX.to_vec();
    // Header unchanged (parse uses the real offset, stays Ok); only the map
    // entry diverges, by a benign in-bounds delta that introduces no overlap.
    assert!(
        patch_map_entry_offset(&mut m, MAP_TYPE_METHOD_ID_ITEM, real_method_off + 4),
        "fixture must contain a method_ids map entry"
    );
    reseal(&mut m);

    let dex = DexFile::parse(&m, None).expect("header still points at real sections → Ok");
    let findings = collect_header_map_findings(&dex);
    let hit = findings.iter().find(|f| {
        f.id == FINDING_ID_DEX_HEADER_MAP_OFFSET_DISAGREEMENT && f.detail.contains("method_ids")
    });
    let hit = hit.unwrap_or_else(|| {
        panic!("expected a method_ids offset-disagreement finding; got {findings:?}")
    });
    assert_eq!(hit.severity, Severity::High, "offset disagreement is High");
}

/// Paired benign/adversarial fixtures, both built from the same source
/// (`fixtures/adversarial/section_offset_overlap/src.java`, class
/// `SectionOverlap`; see that directory's `README.md` for the build recipe +
/// the cross-tool differential against dexdump/jadx). This encodes the honest
/// scope as one hermetic test: the SAME parser accepts the benign layout with a
/// correct inventory (`marker` is a FIELD, `sensitive` is a method) AND
/// hard-rejects the one-field-aliased variant (`method_ids_off := field_ids_off`)
/// as `SectionOverlap`. The committed `.dex` blobs lock the behavior in even if
/// the SDK-dependent generator can't run.
#[test]
fn section_overlap_paired_benign_accepted_adversarial_rejected() {
    // Benign variant: parses Ok with the true inventory.
    let base = include_bytes!("fixtures/adversarial/section_offset_overlap/base.dex");
    let dex = DexFile::parse(base, None).expect("base.dex parses");
    assert!(
        dex.parse_errors.is_empty(),
        "base.dex parse_errors: {:?}",
        dex.parse_errors
    );
    let name = |idx: droidsaw_dex::ids::StringIdx| -> &str {
        dex.strings
            .get(idx.0 as usize)
            .map(|s| s.as_str_lossy())
            .unwrap_or("<oob>")
    };
    let method_names: Vec<&str> = dex.methods.iter().map(|m| name(m.name_idx)).collect();
    let field_names: Vec<&str> = dex.fields.iter().map(|f| name(f.name_idx)).collect();
    assert!(
        method_names.contains(&"sensitive") && method_names.contains(&"exec"),
        "benign inventory must list `sensitive` + `Runtime.exec`: {method_names:?}"
    );
    assert!(
        field_names.contains(&"marker") && !method_names.contains(&"marker"),
        "`marker` must be a FIELD, never a method, in the benign file: fields={field_names:?} methods={method_names:?}"
    );

    // Adversarial variant: the same parser hard-rejects the aliased layout.
    let overlap =
        include_bytes!("fixtures/adversarial/section_offset_overlap/method_ids_aliases_field_ids.dex");
    let r = DexFile::parse(overlap, None);
    assert!(
        matches!(r, Err(DexError::SectionOverlap { .. })),
        "overlap fixture must hard-reject as SectionOverlap, got {r:?}"
    );
}

proptest! {
    /// No-panic invariant on the new overlap path: an arbitrary `method_ids_off`
    /// — in-bounds, out-of-bounds, aliasing, or partially overlapping any other
    /// section — must leave `DexFile::parse` returning a typed result, never a
    /// panic. proptest fails the case on any panic, so the assertion is implicit
    /// in completing the call; the explicit check just pins that a value landing
    /// on the field_ids region is rejected as overlap (not silently accepted).
    #[test]
    fn arbitrary_method_ids_off_never_panics(off in any::<u32>()) {
        let mut m = DEX.to_vec();
        let field_off = get_u32(&m, OFF_FIELD_IDS_OFF);
        put_u32(&mut m, OFF_METHOD_IDS_OFF, off);
        reseal(&mut m);
        let r = DexFile::parse(&m, None);
        if off == field_off {
            prop_assert!(
                matches!(r, Err(DexError::SectionOverlap { .. })),
                "aliasing field_ids must be SectionOverlap, got {r:?}"
            );
        }
        // Any other value: the only requirement is no panic — reaching here is
        // the assertion.
    }
}
