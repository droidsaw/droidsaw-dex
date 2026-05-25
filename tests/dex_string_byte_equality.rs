//! Byte-equality invariant for `DexString::raw_bytes()`.
//!
//! Asserts: for every parsed DEX, every string-pool entry's
//! `raw_bytes()` equals the bytes between `str_start` and `str_end`
//! computed by an **independent** ULEB128 + NUL-scan oracle that
//! does not call `decode_mutf8`. Locks the invariant that the
//! single-pass walker (`parse_string_pool`) produces byte-identical
//! `raw_bytes` to the pre-refactor two-walker arrangement
//! (`parse_strings` + `parse_string_raw_bytes`).
//!
//! Pre-refactor this invariant was tacit — there were two walkers,
//! both reading the same bytes, with no test cross-checking that
//! they agreed. Post-refactor the invariant is structural (one
//! walker stores both decode result and bytes in the same variant)
//! but the oracle test ensures the unification didn't drift from
//! the on-disk byte locus.

use droidsaw_dex::DexFile;

/// Independent oracle: walk the `string_ids` table by reading the
/// raw header bytes, skipping the ULEB128 prefix, and scanning for
/// the trailing NUL. Returns the byte slice for each string-pool
/// entry. No `decode_mutf8` call.
fn oracle_extract_raw_bytes(data: &[u8]) -> Vec<&[u8]> {
    // Header fields are at fixed offsets (DEX spec §7.3).
    fn u32_le(data: &[u8], off: usize) -> u32 {
        let b = &data[off..off + 4];
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
    let string_ids_size = u32_le(data, 56) as usize;
    let string_ids_off = u32_le(data, 60) as usize;

    fn read_uleb128_skip(data: &[u8], mut off: usize) -> usize {
        loop {
            let b = data[off];
            off += 1;
            if b & 0x80 == 0 {
                return off;
            }
        }
    }

    let mut out: Vec<&[u8]> = Vec::with_capacity(string_ids_size);
    for i in 0..string_ids_size {
        let id_off = string_ids_off + i * 4;
        let string_data_off = u32_le(data, id_off) as usize;
        let str_start = read_uleb128_skip(data, string_data_off);
        let str_end = data[str_start..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| str_start + p)
            .unwrap_or(data.len());
        out.push(&data[str_start..str_end]);
    }
    out
}

/// Test fixture: walks classes.dex through `DexFile::parse` and
/// the independent oracle, asserts byte-equality on every entry.
#[test]
fn dex_string_raw_bytes_matches_independent_nul_scan_oracle_on_classes_dex() {
    let data = include_bytes!("fixtures/classes.dex");
    let dex = DexFile::parse(data, None).expect("classes.dex parses");
    let oracle = oracle_extract_raw_bytes(data);
    assert_eq!(
        dex.strings.len(),
        oracle.len(),
        "string count must match between parse and oracle"
    );
    for (i, (entry, expected)) in dex.strings.iter().zip(oracle.iter()).enumerate() {
        assert_eq!(
            entry.raw_bytes(),
            *expected,
            "string[{i}] raw_bytes must equal NUL-scan oracle"
        );
    }
}

#[test]
fn dex_string_raw_bytes_matches_independent_nul_scan_oracle_on_classes_named_dex() {
    let data = include_bytes!("fixtures/classes_named.dex");
    let dex = DexFile::parse(data, None).expect("classes_named.dex parses");
    let oracle = oracle_extract_raw_bytes(data);
    assert_eq!(dex.strings.len(), oracle.len());
    for (i, (entry, expected)) in dex.strings.iter().zip(oracle.iter()).enumerate() {
        assert_eq!(
            entry.raw_bytes(),
            *expected,
            "string[{i}] raw_bytes must equal NUL-scan oracle"
        );
    }
}

/// Walks an adversarial fixture (lone surrogate) to ensure the
/// byte-equality invariant holds even on `MalformedMutf8` entries —
/// the case where the decoded view diverges from the raw bytes.
#[test]
fn dex_string_raw_bytes_matches_oracle_on_lone_surrogate_fixture() {
    let data =
        include_bytes!("fixtures/adversarial/string_length_disagree/lone_surrogate.dex");
    let dex = DexFile::parse(data, None).expect("lone-surrogate fixture parses");
    let oracle = oracle_extract_raw_bytes(data);
    assert_eq!(dex.strings.len(), oracle.len());
    for (i, (entry, expected)) in dex.strings.iter().zip(oracle.iter()).enumerate() {
        assert_eq!(
            entry.raw_bytes(),
            *expected,
            "string[{i}] raw_bytes must equal NUL-scan oracle even on MalformedMutf8"
        );
    }
    // The lone-surrogate fixture has at least one MalformedMutf8 entry;
    // confirm we exercised the path.
    assert!(
        dex.strings.iter().any(|s| s.is_lossy()),
        "lone-surrogate fixture must produce at least one MalformedMutf8 entry"
    );
}
