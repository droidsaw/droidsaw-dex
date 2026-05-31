#![no_main]

//! `fuzz_diag_collectors` — DEX audit-envelope walker family panic gate.
//!
//! Drives bytes through `DexFile::parse`; on parse success, invokes each
//! `pub fn collect_*_findings` walker in `droidsaw_dex::diag` and asserts
//! a per-walker output bound. The existing `fuzz_parser` target already
//! exercises three of these walkers under its Inv 3 (`header_map`,
//! `string_length`, `code_item`); this target re-runs those three for
//! family-wide triage and adds the three not previously fuzzed:
//! `unrecognized`, `duplicate_class_def`, `detector_indeterminate`.
//!
//! **Asserts (on any input where `DexFile::parse` succeeds):**
//!
//! 1. Every walker returns without panic. Internal `unwrap`/`expect`/
//!    `panic!` regressions on the walker family — including the
//!    `format!`/`Vec::push` paths the per-walker unit tests cover under
//!    single-input scenarios — surface here under random parse-success
//!    input.
//!
//! 2. Per-walker output is bounded by an input-derived structural cap:
//!    - `header_map` ≤ 7 (six section pairs `string_ids` … `class_defs`,
//!      mutually exclusive with the single `MAP_LIST_UNREADABLE`
//!      early-return finding; 7 is a defensive ceiling).
//!    - `string_length` ≤ `strings.len() + 1` (up to one
//!      `MISSING_TERMINATOR` finding per string + at most one rollup
//!      `DISAGREEMENT` finding).
//!    - `code_item` ≤ `sum(code.invariant_violations.len())` over
//!      `code_items.values()` (the walker emits exactly one finding per
//!      already-recorded invariant violation).
//!    - `unrecognized` ≤ 1 (single umbrella finding aggregating per-
//!      method Stmt walks; the only non-empty return path is
//!      `vec![finding]`).
//!    - `duplicate_class_def` ≤ `parse_errors.len()` (one finding per
//!      `ParseFailureKind::DuplicateClassDef` record).
//!    - `detector_indeterminate` ≤ `parse_errors.len()` (one finding
//!      per in-scope `ParseFailureKind` record).
//!
//! The UTF-8 invariant on `Finding::detail` is type-system enforced
//! (`String` cannot hold non-UTF-8 unless constructed via
//! `from_utf8_unchecked`, which no walker in `diag.rs` uses) — not
//! asserted here.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(dex) = droidsaw_dex::DexFile::parse(data, None) else {
        return;
    };

    let parse_errors_len = dex.parse_errors.len();
    let strings_len = dex.strings.len();

    // Sum of `CodeItem::invariant_violations.len()` across every parsed
    // `code_item`. This is the exact upper bound on
    // `collect_code_item_findings` — the walker emits one Finding per
    // violation entry; nothing more, nothing less.
    let code_item_violations_total: usize = dex
        .code_items
        .values()
        .map(|ci| ci.invariant_violations.len())
        .fold(0usize, |acc, n| acc.saturating_add(n));

    let hm = droidsaw_dex::diag::collect_header_map_findings(&dex);
    assert!(
        hm.len() <= 7,
        "collect_header_map_findings emitted {} findings; structural cap is 7",
        hm.len(),
    );

    let sl = droidsaw_dex::diag::collect_string_length_findings(&dex);
    let sl_cap = strings_len.saturating_add(1);
    assert!(
        sl.len() <= sl_cap,
        "collect_string_length_findings emitted {} > strings.len() + 1 ({})",
        sl.len(),
        sl_cap,
    );

    let ci = droidsaw_dex::diag::collect_code_item_findings(&dex);
    assert!(
        ci.len() <= code_item_violations_total,
        "collect_code_item_findings emitted {} > sum(invariant_violations.len()) {}",
        ci.len(),
        code_item_violations_total,
    );

    let un = droidsaw_dex::diag::collect_unrecognized_findings(&dex, data);
    assert!(
        un.len() <= 1,
        "collect_unrecognized_findings emitted {} findings; spec is single umbrella (≤ 1)",
        un.len(),
    );

    let dc = droidsaw_dex::diag::collect_duplicate_class_def_findings(&dex);
    assert!(
        dc.len() <= parse_errors_len,
        "collect_duplicate_class_def_findings emitted {} > parse_errors.len() {}",
        dc.len(),
        parse_errors_len,
    );

    let di = droidsaw_dex::diag::collect_detector_indeterminate_findings(&dex);
    assert!(
        di.len() <= parse_errors_len,
        "collect_detector_indeterminate_findings emitted {} > parse_errors.len() {}",
        di.len(),
        parse_errors_len,
    );

    let si = droidsaw_dex::diag::collect_spec_invariant_findings(&dex);
    assert!(
        si.len() <= parse_errors_len,
        "collect_spec_invariant_findings emitted {} > parse_errors.len() {}",
        si.len(),
        parse_errors_len,
    );
});
