//! `fuzz_protector_recognizer` — adversarial-input gate for the
//! protector signature dispatch (SignatureIds 200..299).
//!
//! **Asserts:**
//! 1. No panic for any input that parses. (Panic-freedom invariant.)
//! 2. **Finding structural validity:** every returned `Finding` has a
//!    non-empty `id` and a non-empty `detail`. An empty `id` would
//!    make the finding machine-unreadable; an empty `detail` would
//!    make it human-unreadable. Both are structural bugs.
//! 3. **Count bound:** the number of findings does not exceed
//!    `class_defs.len()`. Each finding originates from a class walk;
//!    more findings than classes indicates the walk produced phantom
//!    entries.
//!
//! This exercises:
//!   * `fragmented_string_literal` (SignatureId 200) — `Stmt::StringConcat`
//!     with all-`Literal` parts.
//!   * `reflective_invoke_stub` (SignatureId 201) — `Method.invoke` plus
//!     upstream binder-stub literal.
//!
//! Adversarial-input discipline: the seed corpus deliberately includes
//! bytes that LOOK protector-shaped but aren't — a clean javac-compiled
//! fixture, a clean reflection chain without binder stubs, a
//! `StringConcat` with mixed literal+var parts. The recognizer must
//! reject those (return `NoMatch` or `NearMiss`, never `Recognized`
//! without evidence) and never panic.
//!
//! Upgraded from panic-only to structural-invariant.

#![no_main]

use libfuzzer_sys::fuzz_target;

use droidsaw_dex::diag::collect_unrecognized_findings;
use droidsaw_dex::parser::DexFile;

fuzz_target!(|data: &[u8]| {
    let Ok(dex) = DexFile::parse(data, None) else {
        // Unparseable input is not a recognizer-pipeline violation;
        // the contract only constrains the diag walk on parser-
        // accepted DEX. (Parse-side panics are the
        // `fuzz_parser` target's invariant, not ours.)
        return;
    };
    let class_def_count = dex.class_defs.len();

    // Drives parse → CFG → SSA → infer → optimize → structure →
    // wrap_try_catch → desugar → run_signatures → walk_unrecognized.
    // Per-method failures inside `build_method_stmt` are absorbed
    // (returns None and skips); reaching the `signature_table()`
    // dispatch — including the protector recognizers — happens for
    // every method that survives the upstream stages.
    let findings = collect_unrecognized_findings(&dex, data);

    // Inv 2: every finding has non-empty id and detail.
    for (i, f) in findings.iter().enumerate() {
        assert!(
            !f.id.is_empty(),
            "finding[{i}] has empty id — machine-unreadable",
        );
        assert!(
            !f.detail.is_empty(),
            "finding[{i}] (id={}) has empty detail — human-unreadable",
            f.id,
        );
    }

    // Inv 3: finding count bounded by class_defs count.
    // Each unrecognized finding originates from one class; phantom
    // entries would exceed this bound.
    assert!(
        findings.len() <= class_def_count,
        "collect_unrecognized_findings returned {} findings but \
         class_defs.len() == {} — walker produced phantom entries",
        findings.len(),
        class_def_count,
    );
});
