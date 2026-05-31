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
const OFF_METHOD_IDS_OFF: usize = 92;

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

/// RED until aliased-section-offset detection lands.
///
/// `field_id_item` and `method_id_item` share an 8-byte on-disk shape, so
/// repointing `method_ids_off` at the `field_ids` region (offset only — sizes
/// untouched, so no size disagreement) makes the same bytes parse as both. The
/// parser currently returns `Ok` of a wrong method table with no `Err`, no
/// `parse_errors`, and no Finding — the header/map cross-check compares sizes
/// only and never inspects offsets. This asserts the desired invariant — the
/// aliasing is surfaced — and flips to passing once the offset cross-check is
/// added; un-ignore it then.
#[test]
#[ignore = "RED: DexFile::parse accepts aliased id-section offsets with no signal; un-ignore when the offset cross-check lands"]
fn aliased_method_ids_offset_is_surfaced() {
    let base = DexFile::parse(DEX, None).expect("baseline fixture parses");
    let field_off = base.header.field_ids_off;

    let mut m = DEX.to_vec();
    put_u32(&mut m, OFF_METHOD_IDS_OFF, field_off); // method_ids_off := field_ids_off
    reseal(&mut m);

    let dex = DexFile::parse(&m, None).expect("tolerant parse stays Ok");
    let findings = droidsaw_dex::diag::collect_header_map_findings(&dex);
    let surfaced = !dex.parse_errors.is_empty()
        || findings.iter().any(|f| f.detail.contains("method_ids"));
    assert!(
        surfaced,
        "aliased method_ids_off must be surfaced via parse_errors or a header/map finding; \
         parse_errors={:?} findings={findings:?}",
        dex.parse_errors
    );
}
