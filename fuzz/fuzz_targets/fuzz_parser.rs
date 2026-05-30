#![no_main]

//! `fuzz_parser` — DEX parser structural-invariant gate.
//!
//! **Asserts (on any input where `DexFile::parse` succeeds):**
//! 1. `type_descriptors.len() <= strings.len()` — every type descriptor
//!    is a string pool reference; more type entries than strings is
//!    structurally impossible per the DEX spec (§3.2 TypeId item).
//! 2. For every `DexString::MalformedMutf8` entry,
//!    `lossy_str == String::from_utf8_lossy(&raw_bytes)`. The type
//!    system enforces it at construction via `new_malformed_mutf8`, but
//!    the `#[non_exhaustive]` guard only blocks _external_ direct-
//!    literal construction; in-crate regressions could silently violate
//!    the invariant — this fuzz arm pins it under random input.
//! 3. Diag collection pipeline completes without panic.
//! 4. **No silent stripping of malformed class_def subsections.** For
//!    every class_def whose `interfaces_off` / `annotations_off` /
//!    `class_data_off` / `static_values_off` is non-zero and past EOF,
//!    `dex.parse_errors` must hold a record at that offset (the
//!    `Interfaces` / `AnnotationDirectory` / `ClassData` /
//!    `EncodedArray` kind respectively). Without this gauge, an
//!    attacker can hide bytes past EOF by pointing one of the four
//!    offsets there; the parser drops the malformed subsection, then
//!    emit produces a "clean" DEX that has lost the hidden content —
//!    a corpus-laundering evasion primitive. The defense is single-
//!    chokepointed at the emit side's `parse_errors.is_empty()` gate;
//!    this assertion gauges that the parse-time side feeds it
//!    correctly. (Symmetry test counterparts live at
//!    `tests/parse_errors_observability.rs`.)
//!
//! Covers the "Parser never panics on random bytes" P0 row.
//!
//! **History.** Pre-refactor (before the 5-parallel-`Vec`-to-`Vec<DexString>`
//! collapse), this target asserted parallel-pool length equality across
//! `string_raw_bytes` / `lossy_decode_marks` / `declared_string_lengths` /
//! `missing_terminator_marks`. That invariant is now type-system enforced
//! by-construction: each `DexString` carries the bytes, decode result,
//! declared char count, and terminator gauge in the same variant; there
//! are no parallel arrays left to desync. The pre-refactor Inv 1 is
//! preserved as a tautology by the schema and not asserted here.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(dex) = droidsaw_dex::DexFile::parse(data, None) else {
        return;
    };

    let n = dex.strings.len();

    // Inv 1: type_descriptors.len() <= strings.len() — type descriptors are
    // string references; more type entries than string pool entries is impossible
    // in a spec-valid DEX (each TypeId indexes into the string pool).
    assert!(
        dex.type_descriptors.len() <= n,
        "type_descriptors.len() ({}) > strings.len() ({}) — impossible per DEX spec",
        dex.type_descriptors.len(),
        n,
    );

    // Inv 2: `lossy_str ≡ from_utf8_lossy(raw_bytes)` invariant on
    // `MalformedMutf8`. Constructor enforces this at parse time; this
    // arm pins it under fuzz so an in-crate direct-literal construction
    // that bypassed `new_malformed_mutf8` would surface here rather than
    // as a downstream display-vs-scan divergence.
    for entry in &dex.strings {
        if let droidsaw_dex::DexString::MalformedMutf8 { .. } = entry {
            let recomputed = String::from_utf8_lossy(entry.raw_bytes());
            assert_eq!(
                entry.as_str_lossy(),
                recomputed.as_ref(),
                "MalformedMutf8 lossy_str invariant violated: stored != from_utf8_lossy(raw_bytes)",
            );
        }
    }

    // Inv 3: diag pipeline completes without panic.
    let _ = droidsaw_dex::diag::collect_header_map_findings(&dex);
    let _ = droidsaw_dex::diag::collect_string_length_findings(&dex);
    let _ = droidsaw_dex::diag::collect_code_item_findings(&dex);

    // Inv 4: no silent stripping of malformed class_def subsections.
    // For every class_def whose subsection offset is non-zero and past
    // EOF, `parse_errors` must hold a record at that offset with the
    // expected kind. Closes the evasion primitive: an attacker who
    // points a class_def subsection past EOF cannot get a clean
    // emit-round-trip from droidsaw, because emit refuses any DEX with
    // non-empty `parse_errors`.
    use droidsaw_dex::parser::ParseFailureKind;
    let data_len = data.len() as u32;
    for cd in &dex.class_defs {
        for (off, expected_kind) in [
            (cd.interfaces_off, ParseFailureKind::Interfaces),
            (cd.annotations_off, ParseFailureKind::AnnotationDirectory),
            (cd.class_data_off, ParseFailureKind::ClassData),
            (cd.static_values_off, ParseFailureKind::EncodedArray),
        ] {
            if off == 0 || off <= data_len {
                continue;
            }
            let recorded = dex.parse_errors.iter().any(|pf| {
                pf.offset == off
                    && std::mem::discriminant(&pf.kind) == std::mem::discriminant(&expected_kind)
            });
            assert!(
                recorded,
                "evasion-primitive gate: class_def offset {off:#x} past EOF (data_len={data_len}) \
                 must produce a {expected_kind:?} ParseFailure record; got parse_errors = {:?}",
                dex.parse_errors,
            );
        }
    }

    // Inv 5 (§H-1): pool-ordering records are exact. The parser records
    // a single `OutOfOrder { pool }` iff that index-keyed pool is not
    // strictly ascending by its DEX-spec sort key (which also catches
    // duplicates). Cross-checked here with an INDEPENDENT is-sorted scan
    // for `field_ids` / `method_ids`, whose keys are plain index tuples
    // on `DexFile` — genuinely independent of the parser's
    // `first_inversion`. The `type_ids` / `proto_ids` keys (raw
    // descriptor index / resolved parameter list) are not retained on
    // `DexFile`, so they are gauged by the in-crate unit tests instead.
    use droidsaw_dex::parser::PoolKind;
    let has_oo = |pool: PoolKind| {
        dex.parse_errors
            .iter()
            .any(|pf| pf.kind == ParseFailureKind::OutOfOrder { pool })
    };
    let fields_sorted = dex.fields.windows(2).all(|w| {
        (w[0].class_idx, w[0].name_idx, w[0].type_idx)
            < (w[1].class_idx, w[1].name_idx, w[1].type_idx)
    });
    assert_eq!(
        has_oo(PoolKind::FieldIds),
        !fields_sorted,
        "field_ids OutOfOrder record must match independent is-sorted scan",
    );
    let methods_sorted = dex.methods.windows(2).all(|w| {
        (w[0].class_idx, w[0].name_idx, w[0].proto_idx)
            < (w[1].class_idx, w[1].name_idx, w[1].proto_idx)
    });
    assert_eq!(
        has_oo(PoolKind::MethodIds),
        !methods_sorted,
        "method_ids OutOfOrder record must match independent is-sorted scan",
    );

    // Inv 6 (§H-5): the typed super-chain terminator always terminates
    // (bounded by MAX_SUPER_CHAIN_DEPTH) and never panics on an
    // adversarial — possibly cyclic — class hierarchy. Exercise it for
    // every class. The topological-ordering check itself runs inside
    // DexFile::parse, so a planted out-of-order class hierarchy is
    // already gauged by the parse above.
    for cd in &dex.class_defs {
        let _ = dex.super_class_chain_terminator(cd.class_idx);
    }
});
