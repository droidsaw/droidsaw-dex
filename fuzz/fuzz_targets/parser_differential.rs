#![no_main]

// Differential oracle fuzz target for DEX parser.
//
// Property: for every raw byte slice where production succeeds,
// `naive_parse_dex(data).to_shape()` must equal `DexFile::parse(data, None).to_shape()`.
//
// Any shape divergence is a silent-wrong-parse bug at layer 1 of the
// layered-oracle architecture. The oracle catches bugs that panic-fuzz
// and ASAN cannot: wrong-answer-that-doesn't-crash.
//
// Invariants asserted:
// 1. naive_parse_dex(data) == DexFile::parse(data, None).to_shape()
//    (on any input where BOTH parsers succeed)
// 2. If production accepts, oracle must also accept
//    (oracle must not be more restrictive than production)
//
// Oracle-more-permissive (oracle accepts, production rejects) is NOT a
// hard assertion: production performs Adler-32 checksum verification,
// cross-reference index validation, and semantic checks that a structural
// oracle intentionally skips. The oracle's purpose is to detect wrong
// answers on production-accepted inputs, not to replicate all of
// production's rejection conditions.
//
// Harness design:
// - If both fail:              skip (agreement by rejection).
// - If both succeed:           assert ParseShape equality.
// - If prod accepts, oracle rejects: panic (oracle too strict).
// - If oracle accepts, prod rejects: skip (oracle more permissive; expected).
// - Stateless: no internal mutation across fuzz iterations.

use droidsaw_dex::parser::DexFile;
use droidsaw_dex::parser_oracle::naive_parse_dex;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let prod = DexFile::parse(data, None);
    let oracle = naive_parse_dex(data);

    match (&prod, &oracle) {
        // Both fail — no shape to compare; both agreed (both rejected). OK.
        (Err(_), Err(_)) => {}

        // Both succeed — assert ParseShape equality.
        (Ok(prod_file), Ok(oracle_shape)) => {
            let prod_shape = prod_file.to_shape();
            assert_eq!(
                prod_shape,
                *oracle_shape,
                "ParseShape DIVERGED on production-accepted input\n\
                 production: {prod_shape:#?}\n\
                 oracle:     {oracle_shape:#?}"
            );
        }

        // Production accepted but oracle rejected — oracle is more restrictive.
        // Hard assertion: the oracle must agree with production on any
        // input production accepts.
        (Ok(prod_file), Err(oracle_err)) => {
            let prod_shape = prod_file.to_shape();
            panic!(
                "ORACLE-REJECTED what production accepted\n\
                 oracle_err: {oracle_err:?}\n\
                 prod_shape.string_ids_size={}, .type_ids_size={}, .class_defs_size={}",
                prod_shape.string_ids_size,
                prod_shape.type_ids_size,
                prod_shape.class_defs_size,
            );
        }

        // Oracle accepted but production rejected — oracle is more permissive.
        // Not a hard assertion: production validates checksums, cross-reference
        // indices, and semantic constraints that a structural oracle intentionally
        // skips. Log via std::hint::black_box so libFuzzer sees the branch
        // without triggering a crash.
        (Err(_prod_err), Ok(_oracle_shape)) => {
            // Structural divergence: oracle is more permissive than production.
            // Expected for inputs rejected by Adler-32, IndexOob, file_size
            // semantic checks, etc. The fuzz corpus grows; this arm is only a
            // divergence signal if it appears on inputs with non-zero shape fields.
        }
    }
});
