#![no_main]

//! Fuzz target for `droidsaw_dex::debug::parse_debug_info` — the DEX
//! debug_info_item ULEB128 opcode-stream state machine.
//!
//! Sibling pattern: `droidsaw_hermes/fuzz/fuzz_targets/fuzz_decode_source_locations.rs`.
//!
//! Outer shape: `(registers_size_byte: u8, payload: &[u8])` packed into the
//! libFuzzer `&[u8]` slot. The first byte selects `registers_size` via
//! `(byte & 0x7F) as u16` — yields the full range `0..=127` including the
//! degenerate `registers_size = 0` case (intentionally exercised).
//!
//! Named bug-shapes hunted:
//! - **DBG-B1**: signed-SLEB128 overflow in `DBG_ADVANCE_LINE` state.
//! - **DBG-B2**: `DBG_RESTART_LOCAL` referencing a never-started register.
//! - **DBG-B3**: OOB register index when `registers_size` is caller-derived
//!   (this harness exercises the full registers_size range including 0).
//! - **DBG-B4** (state-interaction): opcode sequences whose individual
//!   opcodes are well-formed but whose stream violates IR invariants
//!   (e.g. RESTART_LOCAL after END_SEQUENCE).
//!
//! Invariants asserted per iteration:
//!   1. No panic on any input regardless of `registers_size`.
//!   2. **Locals well-formedness** (gauges the `narrow_register` Kani
//!      guarantee from prior hardening):
//!      every entry in `DebugInfo.locals` has `register < registers_size`.
//!   3. **Line-table monotonicity-or-reset**: `DBG_ADVANCE_LINE` is signed;
//!      after parse, every line-table entry's line value is non-negative,
//!      and successive entries either advance the address strictly or
//!      follow a sequence reset. The exact shape varies per spec; the
//!      coarse assertion is "no internally contradictory state-machine
//!      result emitted to the IR."
//!   4. Determinism: a second call on the same input produces an equal
//!      `Result`.
//!
//! Cross-reference: the `narrow_register` Kani proof at
//! `proofs/debug_register_bound.rs` covers the register-narrowing bound
//! at the type level; this fuzz target covers the state-machine
//! dispatch + line/address-advance arithmetic + active-locals map state
//! interactions that Kani cannot reach.

use droidsaw_dex::debug::parse_debug_info;
use droidsaw_dex::parser::DexFile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let registers_size: u16 = u16::from(data[0] & 0x7F);
    let payload = &data[1..];

    // Empty DexFile — debug-info state-machine string-resolution lookups
    // return None against this instance, which the state machine treats
    // as "name unavailable for this index" (the legitimate runtime case
    // for stripped DEXes). Sub-parser fuzz: the dex param is the minimum
    // surface needed to invoke the state machine; we don't want the dex
    // param's shape to dominate mutation reach.
    let dex = DexFile::empty_for_fuzz();

    let result = parse_debug_info(payload, 0, registers_size, &dex);

    if let Ok(ref info) = result {
        // (2) Locals well-formedness — every locals entry's register
        //     must be `< registers_size` per DEX §3.4.6. Closes the
        //     register-narrowing regression surface from the
        //     `dex-debug-info-register-uleb-truncation` fix.
        for (idx, local) in info.locals.iter().enumerate() {
            assert!(
                local.register < registers_size,
                "locals[{idx}].register={} >= registers_size={registers_size}",
                local.register
            );
        }

        // (2b) Degenerate-`registers_size=0` gauge — when no registers
        //      are declared, the parser must not emit any locals. Closes
        //      the gauge-hole the register<registers_size assert leaves
        //      open (the assert is `register < 0` which is unsatisfiable
        //      for u16 and trivially passes regardless of locals.len()).
        if registers_size == 0 {
            assert!(
                info.locals.is_empty(),
                "registers_size=0 must produce empty locals, got {} entries",
                info.locals.len()
            );
        }

        // (3) Line-table monotonicity — production claims "Monotonic in pc
        //     by construction" (debug.rs:31). The state machine produces
        //     (addr, line) entries in PC order, but the same PC can have
        //     multiple line records (multi-line-per-pc rows), so the right
        //     gauge is `addr non-decreasing` (not strict-greater). A
        //     violation here is a state-machine arithmetic bug
        //     (DBG_ADVANCE_PC delta underflow producing a wrap-around).
        {
            let mut prev_addr: u32 = 0;
            for (i, (addr, _line)) in info.line_table.iter().enumerate() {
                if i > 0 {
                    assert!(
                        *addr >= prev_addr,
                        "line_table[{i}].addr={addr} < prev={prev_addr}; \
                         monotonicity violation (DBG_ADVANCE_PC arithmetic \
                         produced an effective advance underflow)"
                    );
                }
                prev_addr = *addr;
            }
        }

    }

    // (4) Determinism — same input must produce the same Result.
    //     Full Ok-side structural equality (DebugInfo derives PartialEq);
    //     Err-side discriminant equality (DexError does NOT derive
    //     PartialEq — 228-line enum with non-trivial variants; using
    //     mem::discriminant captures variant-flip nondeterminism without
    //     requiring a broader-fanout PartialEq derive). This is the same
    //     intent as the hermes sibling's `assert_eq!(result, result2)`
    //     adapted to the Result<DebugInfo, DexError> shape.
    let result2 = parse_debug_info(payload, 0, registers_size, &dex);
    match (&result, &result2) {
        (Ok(a), Ok(b)) => assert_eq!(
            a, b,
            "parse_debug_info nondeterministic Ok contents on \
             registers_size={registers_size} payload.len={}",
            payload.len()
        ),
        (Err(e1), Err(e2)) => assert_eq!(
            core::mem::discriminant(e1),
            core::mem::discriminant(e2),
            "parse_debug_info nondeterministic Err variant on \
             registers_size={registers_size} payload.len={}",
            payload.len()
        ),
        _ => panic!(
            "parse_debug_info nondeterministic Ok/Err on \
             registers_size={registers_size} payload.len={}",
            payload.len()
        ),
    }
});
