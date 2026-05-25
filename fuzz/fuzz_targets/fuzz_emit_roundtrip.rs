//! `fuzz_emit_roundtrip` — differential-parse round-trip gate.
//!
//! Invariant under test:
//!   ∀ bytes where `DexFile::parse(bytes, None)` succeeds,
//!   `parse(emit_dex(parse(bytes)))` must succeed and produce
//!   content-equivalent IR to the first parse.
//!
//! `.expect()` (not `.unwrap()`) with context — unwrap would mask the
//! bug class the fuzzer is meant to catch.
//!
//! Content equivalence matches `tests/roundtrip_proptest.rs`:
//! layout-dependent fields (offsets, checksums) are expected to
//! differ across emit; pool contents + presence bits must not.

#![no_main]

use libfuzzer_sys::fuzz_target;

use droidsaw_dex::emit_dex::emit_dex;
use droidsaw_dex::parser::{ContentEquiv, DexFile};

fuzz_target!(|data: &[u8]| {
    // First parse. Unparseable input is not a round-trip violation;
    // the contract only constrains `emit` on parser-accepted IR.
    let Ok(dex1) = DexFile::parse(data, None) else {
        return;
    };

    // Emit. Per the emit_dex module docs "Emit domain ⊊ Parse domain":
    // emit may return typed `UnrepresentableIR` for parse-accepted IR
    // that's structurally ill-formed (e.g., non-canonical string pool
    // order, sentinel collision). Both NotImplemented and
    // UnrepresentableIR are expected "can't handle this input"
    // channels; panic only on UNTYPED errors or panics (which a Rust
    // Result can't express — libfuzzer catches those via unwind).
    let emitted = match emit_dex(&dex1) {
        Ok(buf) => buf,
        Err(droidsaw_dex::emit_dex::DexEmitError::NotImplemented) => return,
        Err(droidsaw_dex::emit_dex::DexEmitError::UnrepresentableIR { .. }) => return,
        Err(droidsaw_dex::emit_dex::DexEmitError::SizeOverflow { .. }) => return,
        Err(droidsaw_dex::emit_dex::DexEmitError::OffsetOverflow { .. }) => return,
        // PartialIR fires when the parser captured a silent skip in
        // `parse_errors`. Fuzz drives mutated inputs, so parse
        // errors are EXPECTED on most samples. Skip — the emit
        // contract is that PartialIR is a typed refusal, not an
        // internal bug. (strict-default gate landed at a2ca788.)
        Err(droidsaw_dex::emit_dex::DexEmitError::PartialIR { .. }) => return,
        Err(e) => panic!("emit_dex internal/unexpected failure: {e}"),
    };

    // Second parse. If the first parse succeeded and emit succeeded,
    // re-parse MUST succeed — otherwise emit produced malformed bytes.
    let dex2 = DexFile::parse(&emitted, None).expect("emitted bytes failed to re-parse");

    // Content equivalence via the quotient newtype: single source of
    // truth for "what counts as round-trip equivalent" lives in
    // parser.rs::ContentEquiv. Adding a new subsection to DexFile
    // only updates ContentEquiv, not this target.
    assert_eq!(
        ContentEquiv(&dex1),
        ContentEquiv(&dex2),
        "content equivalence violated post round-trip"
    );
});
