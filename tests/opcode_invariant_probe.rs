//! Empirical probe for the opcode-invariant matrix.
//!
//! Each test constructs a minimal adversarial byte sequence and feeds it
//! directly to `decode::decode_insns`. The assertion confirms current
//! behavior matches the matrix prediction (see
//! `docs/opcode-invariant-matrix.md` §4).
//!
//! These probes are expected to *change* when the fixes land:
//! the "accept silently" branches become "accept with `Finding` emitted"
//! (tolerant-parse non-negotiable). Each test's `EXPECTED POST-FIX`
//! comment names the planned shape so the diff at fix-time is contained.

use droidsaw_dex::decode::decode_insns;
use droidsaw_dex::opcodes::Opcode;

/// HIGH-1 — F35c arg_count silent clamp 0..=15 → 0..=5.
///
/// Construct `invoke-virtual {v0,v0,v0,v0,v0}, method_id=0` with the B
/// nibble (arg_count) set to 15 instead of the spec-mandated max of 5.
/// Per matrix §4 HIGH-1, the decoder reads the high nibble of unit 0 as
/// arg_count and silently clamps via `.min(5)` at `decode.rs:571`.
///
/// EXPECTED CURRENT: decode succeeds; one Instruction with `src.len() == 5`
/// (the original 15-encoded arg_count is silently truncated; no Finding).
///
/// EXPECTED POST-FIX: decode succeeds; `src.len() == 5`
/// (clamp retained for tolerant-parse); but `Finding` /
/// `CodeItemInvariantViolation::OpcodeArgCountOutOfRange` emitted with
/// observed=15, max=5.
#[test]
fn probe_h1_f35c_arg_count_overflow() {
    // F35c layout (3 code units, little-endian):
    //   Unit 0 high byte: B (arg_count, 4 bits) | A (g-reg, 4 bits)
    //   Unit 0 low byte:  opcode
    //   Unit 1:           method_id (BBBB)
    //   Unit 2 high byte: F (4 bits) | E (4 bits)
    //   Unit 2 low byte:  D (4 bits) | C (4 bits)
    //
    // invoke-virtual = 0x6E. arg_count=15 (B=0xF), all regs = v0.
    let bytes: &[u8] = &[
        0x6E, 0xF0, // op + (B=0xF | A=0x0)
        0x00, 0x00, // method_id = 0
        0x00, 0x00, // F=0 E=0 D=0 C=0
    ];
    let result = decode_insns(bytes, 0, 3);
    let (insns, payloads, violations) = result.expect("H-1: tolerant-parse retains clamp");
    assert_eq!(insns.len(), 1, "H-1: should decode exactly one instruction");
    assert_eq!(insns[0].op, Opcode::InvokeVirtual);
    let observed_arg_count = insns[0].src.len();
    assert_eq!(
        observed_arg_count, 5,
        "H-1: clamp retained (RegList capacity is 5; downstream IR consumers see <=5 regs)"
    );
    assert!(payloads.is_empty(), "H-1: no payloads expected");
    // Typed violation surfaced with pre-clamp observed value.
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    let h1 = violations.iter().find(|v| matches!(
        v,
        CodeItemInvariantViolation::OpcodeArgCountOutOfRange { .. }
    ));
    let h1 = h1.expect("H-1 FINDING EMITTED: OpcodeArgCountOutOfRange variant present");
    match h1 {
        CodeItemInvariantViolation::OpcodeArgCountOutOfRange {
            opcode,
            source_pc,
            observed,
            max,
        } => {
            assert_eq!(*opcode, Opcode::InvokeVirtual, "H-1: opcode preserved");
            assert_eq!(*source_pc, 0, "H-1: source_pc points to the offending insn");
            assert_eq!(*observed, 15, "H-1: pre-clamp observed value preserved");
            assert_eq!(*max, 5, "H-1: ART kMaxVarArgRegs cite");
        }
        _ => unreachable!(),
    }
}

/// MEDIUM-1 — Goto32 branch target accepted with target far outside
/// `insns_size`.
///
/// `goto/32 +0x7FFFFFFF` from pc=0 lands at address 0x7FFFFFFF, which is
/// 2^30 code units beyond any plausible method size. Per matrix §4
/// MEDIUM-1, `branch_target` (`decode.rs:365`) only checks for u32
/// overflow via `checked_add_signed`; in-range against `insns_size` is
/// not enforced at decode.
///
/// EXPECTED CURRENT: decode succeeds; instruction has
/// `target = Some(0x7FFFFFFF)`. CFG layer silently drops the edge at
/// `cfg.rs:256` (`find_leaders`). No Finding emitted.
///
/// EXPECTED POST-FIX: decode succeeds; same target
/// preserved in IR; `Finding` /
/// `CodeItemInvariantViolation::BranchTargetOutOfRange` emitted with
/// source_pc, target, insns_size for analyst visibility.
#[test]
fn probe_m1_branch_target_out_of_range() {
    // F30t Goto32 layout (3 code units, little-endian):
    //   Unit 0 high byte: unused (AA)
    //   Unit 0 low byte:  0x2A (Goto32)
    //   Units 1+2:        i32 signed offset (low half first)
    //
    // offset = 0x7FFFFFFF → bytes (LE): FF FF FF 7F.
    let bytes: &[u8] = &[
        0x2A, 0x00, // op + AA=0
        0xFF, 0xFF, // offset low half
        0xFF, 0x7F, // offset high half (= 0x7FFFFFFF total)
    ];
    let result = decode_insns(bytes, 0, 3);
    let (insns, _, violations) = result.expect("M-1: tolerant-parse retains OOB target");
    assert_eq!(insns.len(), 1, "M-1: should decode exactly one Goto32");
    assert_eq!(insns[0].op, Opcode::Goto32);
    assert_eq!(
        insns[0].target,
        Some(0x7FFFFFFF),
        "M-1: OOB target retained in IR (tolerant-parse)"
    );
    // Violation surfaced.
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    let m1 = violations
        .iter()
        .find(|v| matches!(v, CodeItemInvariantViolation::BranchTargetOutOfRange { .. }))
        .expect("M-1 FINDING EMITTED: BranchTargetOutOfRange variant present");
    match m1 {
        CodeItemInvariantViolation::BranchTargetOutOfRange {
            opcode,
            source_pc,
            target,
            insns_size,
        } => {
            assert_eq!(*opcode, Opcode::Goto32);
            assert_eq!(*source_pc, 0);
            assert_eq!(*target, 0x7FFFFFFF);
            assert_eq!(*insns_size, 3);
        }
        _ => unreachable!(),
    }
}

/// MEDIUM-2 — packed-switch pointing at a fill-array-data payload is
/// dispatched by source opcode, not by payload ident byte. Two parser
/// passes over the same bytes under different grammars without
/// reconciliation.
///
/// Layout: PackedSwitch at pc=0 with target=+4 (so payload_pc=4, even/aligned).
/// At pc=4 we place a fill-array-data payload (ident=0x0300, width=1, size=0).
/// The skip-walker sees ident=0x03 and computes fill-array-data payload size.
/// The resolve-walker sees source opcode=PackedSwitch and dispatches
/// `decode_packed_switch` on the same bytes, mis-parsing them.
///
/// Note: offset=+4 (even) is used here to avoid triggering BONUS-4
/// `UnalignedTableDexPc`. The 4-unit NOP between the switch and payload is
/// a 2-unit `goto/32` followed by a 2-unit NOP to reach an even payload address.
/// Simpler: insert a 1-unit NOP at pc=3 so payload lands at pc=4 (even).
///
/// EXPECTED POST-FIX: decode reads payload ident u16
/// first, validates against source opcode, emits
/// `CodeItemInvariantViolation::PayloadIdentMismatch`. Tolerant-parse:
/// payload dropped from the map; CFG already handles missing payloads.
#[test]
fn probe_m2_payload_ident_mismatch() {
    // pc=0 (bytes 0-5): PackedSwitch with offset=+4 → payload_pc=4 (even).
    //   Unit 0: opcode=0x2B + AA=0
    //   Units 1+2: i32 offset = +4
    // pc=3 (bytes 6-7): NOP (0x00, 0x00) — bridge to even payload address
    // pc=4 (bytes 8-15): fill-array-data payload
    //   Unit 0: ident=0x0300
    //   Unit 1: element_width=1
    //   Units 2+3: size=0 (u32 LE)
    let bytes: &[u8] = &[
        // PackedSwitch at pc=0
        0x2B, 0x00, // op + AA=0
        0x04, 0x00, // offset low half = 4
        0x00, 0x00, // offset high half = 0 → offset=+4 → target=4
        // NOP at pc=3
        0x00, 0x00, // nop
        // fill-array-data payload at pc=4
        0x00, 0x03, // ident = 0x0300 (fill-array-data)
        0x01, 0x00, // element_width = 1
        0x00, 0x00, 0x00, 0x00, // size = 0
    ];
    // insns_size = 8 code units (PackedSwitch=3 + NOP=1 + fill-array-data=4).
    let result = decode_insns(bytes, 0, 8);
    let (insns, payloads, violations) = result.expect("M-2: tolerant-parse drops mis-typed payload");
    // The switch + NOP instructions are decoded:
    assert_eq!(insns.len(), 2, "M-2: PackedSwitch + NOP decoded");
    assert_eq!(insns[0].op, Opcode::PackedSwitch);
    assert_eq!(insns[0].target, Some(4));
    // Mis-typed payload is DROPPED from the map rather than
    // mis-parsed. CFG-layer-graceful handling at cfg.rs:267 already
    // tolerates a switch with no resolved payload entry.
    assert!(
        !payloads.contains_key(&4),
        "M-2: ident-mismatched payload dropped from payloads map"
    );
    // Violation surfaced.
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    let m2 = violations
        .iter()
        .find(|v| matches!(v, CodeItemInvariantViolation::PayloadIdentMismatch { .. }))
        .expect("M-2 FINDING EMITTED: PayloadIdentMismatch variant present");
    match m2 {
        CodeItemInvariantViolation::PayloadIdentMismatch {
            source_opcode,
            source_pc,
            payload_pc,
            expected_ident,
            observed_ident,
        } => {
            assert_eq!(*source_opcode, Opcode::PackedSwitch);
            assert_eq!(*source_pc, 0);
            assert_eq!(*payload_pc, 4);
            assert_eq!(*expected_ident, 0x0100, "M-2: PackedSwitch expects ident=0x0100");
            assert_eq!(*observed_ident, 0x0300, "M-2: payload's actual ident is 0x0300 (fill-array-data)");
        }
        _ => unreachable!(),
    }
}

/// UNCHECKED-1 — unknown opcode byte (0x73 is unmapped per `Opcode::from_u8`)
/// silently skipped 1 unit. Cursor-misalignment primitive: the bytes
/// after the unknown opcode are interpreted as a different instruction
/// than Dalvik's verifier would see (verifier rejects entirely).
///
/// EXPECTED CURRENT: decode succeeds; the 0x73 byte is skipped; the
/// following bytes decode as ReturnVoid. No Finding emitted.
///
/// EXPECTED POST-FIX: decode emits `Finding` /
/// `CodeItemInvariantViolation::UnknownOpcode` with source_pc, opcode_byte;
/// tolerant-parse continues (skip 1 unit, decode the rest) so downstream
/// CFG sees the same stream the parser produced.
#[test]
fn probe_unchecked1_unknown_opcode_byte() {
    // 0x73 is one of the unmapped opcode bytes — verify via Opcode::from_u8.
    assert!(
        Opcode::from_u8(0x73).is_none(),
        "UNCHECKED-1 precondition: 0x73 must be unmapped"
    );

    // Bytes:
    //   pc=0: 0x73 0x00 — unknown opcode
    //   pc=1: 0x0E 0x00 — ReturnVoid
    let bytes: &[u8] = &[0x73, 0x00, 0x0E, 0x00];
    let result = decode_insns(bytes, 0, 2);
    let (insns, _, violations) = result.expect("UNCHECKED-1: tolerant-parse skips 1 unit");
    assert_eq!(
        insns.len(),
        1,
        "UNCHECKED-1: 0x73 skipped 1 unit; ReturnVoid at pc=1 ends up in IR"
    );
    assert_eq!(insns[0].op, Opcode::ReturnVoid);
    assert_eq!(
        insns[0].addr, 1,
        "UNCHECKED-1: ReturnVoid lands at pc=1 (decoder skipped pc=0 by 1 unit)"
    );
    // Violation surfaced.
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    let u1 = violations
        .iter()
        .find(|v| matches!(v, CodeItemInvariantViolation::UnknownOpcodeByte { .. }))
        .expect("UNCHECKED-1 FINDING EMITTED: UnknownOpcodeByte variant present");
    match u1 {
        CodeItemInvariantViolation::UnknownOpcodeByte {
            source_pc,
            opcode_byte,
        } => {
            assert_eq!(*source_pc, 0);
            assert_eq!(*opcode_byte, 0x73);
        }
        _ => unreachable!(),
    }
}

// ── Bonus invariant probes ──

/// BONUS-1 — `goto` with offset==0 (self-branch tight loop).
///
/// ART rejects via `method_verifier.cc:2181-2184` (FailBranchOffsetZero).
/// `goto/32` (F30t) is exempt; all shorter forms are rejected.
///
/// Layout: `goto` (0x28, F10t) with AA=0 → offset=0 → target=pc=0.
/// insns_size=1 so the instruction fits exactly.
#[test]
fn probe_bonus1_branch_offset_zero() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    // goto (F10t): 1 code unit; AA byte is the signed 8-bit offset.
    // opcode=0x28, AA=0x00 → offset=0 → target=source_pc=0.
    let bytes: &[u8] = &[0x28, 0x00];
    let (insns, _, violations) = decode_insns(bytes, 0, 1)
        .expect("BONUS-1: tolerant-parse retains zero-offset goto");
    assert_eq!(insns.len(), 1);
    assert_eq!(insns[0].op, Opcode::Goto);
    assert_eq!(insns[0].target, Some(0), "BONUS-1: target is self (pc=0)");
    let v = violations
        .iter()
        .find(|v| matches!(v, CodeItemInvariantViolation::BranchOffsetZero { .. }))
        .expect("BONUS-1: BranchOffsetZero violation emitted");
    match v {
        CodeItemInvariantViolation::BranchOffsetZero { opcode, source_pc } => {
            assert_eq!(*opcode, Opcode::Goto);
            assert_eq!(*source_pc, 0);
        }
        _ => unreachable!(),
    }
}

/// BONUS-1b — `goto/32` with offset==0 is ART-exempt (no violation).
///
/// ART's FailBranchOffsetZero exempts F30t; droidsaw must not emit the
/// violation for `goto/32` with a zero offset.
#[test]
fn probe_bonus1b_goto32_zero_offset_exempt() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    // goto/32 (F30t, 0x2A): 3 code units; offset is i32 spanning units 1+2.
    // offset=0 → target=0. insns_size=3.
    let bytes: &[u8] = &[
        0x2A, 0x00, // op + AA=0
        0x00, 0x00, // offset low half = 0
        0x00, 0x00, // offset high half = 0 → offset=0 → target=0
    ];
    let (insns, _, violations) = decode_insns(bytes, 0, 3)
        .expect("BONUS-1b: goto/32 zero-offset is ART-exempt");
    assert_eq!(insns.len(), 1);
    assert_eq!(insns[0].op, Opcode::Goto32);
    // No BranchOffsetZero violation for goto/32.
    let has_zero_offset = violations
        .iter()
        .any(|v| matches!(v, CodeItemInvariantViolation::BranchOffsetZero { .. }));
    assert!(!has_zero_offset, "BONUS-1b: goto/32 is exempt from FailBranchOffsetZero");
}

/// BONUS-2 — branch target lands mid-instruction (inside a 2-unit insn).
///
/// ART rejects via `method_verifier.cc:2192-2195` (FailTargetMidInstruction).
///
/// Layout:
///   pc=0 (units 0-1): `goto/16` (F20t, 0x29) with offset=+1 → target=1
///   pc=2: ignored (outside insns_size=2)
///
/// The `goto/16` at pc=0 spans code units 0 and 1. Its target=1 is
/// mid-instruction (unit 1 of the `goto/16` itself). insns_size=2.
#[test]
fn probe_bonus2_branch_target_mid_instruction() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    // goto/16 (F20t, 0x29, 2 code units): offset in unit 1.
    // Unit 0: opcode=0x29, AA=0x00
    // Unit 1: i16 offset = 1 → target = pc(0) + 1 = 1
    let bytes: &[u8] = &[
        0x29, 0x00, // op + AA
        0x01, 0x00, // offset = +1
    ];
    let (insns, _, violations) = decode_insns(bytes, 0, 2)
        .expect("BONUS-2: tolerant-parse retains mid-instruction branch");
    assert_eq!(insns.len(), 1, "BONUS-2: one goto/16 decoded");
    assert_eq!(insns[0].op, Opcode::Goto16);
    assert_eq!(insns[0].target, Some(1), "BONUS-2: target=1 (mid-instruction)");
    let v = violations
        .iter()
        .find(|v| matches!(v, CodeItemInvariantViolation::BranchTargetMidInstruction { .. }))
        .expect("BONUS-2: BranchTargetMidInstruction violation emitted");
    match v {
        CodeItemInvariantViolation::BranchTargetMidInstruction {
            opcode,
            source_pc,
            target_pc,
            owner_pc,
        } => {
            assert_eq!(*opcode, Opcode::Goto16);
            assert_eq!(*source_pc, 0);
            assert_eq!(*target_pc, 1);
            assert_eq!(*owner_pc, 0, "BONUS-2: owner of unit 1 is the goto/16 at pc=0");
        }
        _ => unreachable!(),
    }
}

/// BONUS-3 — branch target is a `move-result` instruction.
///
/// ART rejects via `method_verifier.cc:2197-2200`
/// (FailBranchTargetIsMoveResultOrMoveException).
///
/// Layout:
///   pc=0 (1 unit): `goto` (F10t) with signed offset=+1 → target=1
///   pc=1 (1 unit): `move-result v0` (0x0A, F11x)
///   insns_size=2
#[test]
fn probe_bonus3_branch_target_is_move_result() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    // goto (0x28, F10t): AA = signed 8-bit offset.
    // offset=+1 → target = 0+1 = 1.
    // move-result v0 (0x0A, F11x): AA=0 (v0).
    let bytes: &[u8] = &[
        0x28, 0x01, // goto +1 → target=1
        0x0A, 0x00, // move-result v0
    ];
    let (insns, _, violations) = decode_insns(bytes, 0, 2)
        .expect("BONUS-3: tolerant-parse retains branch-to-move-result");
    assert_eq!(insns.len(), 2, "BONUS-3: two instructions decoded");
    assert_eq!(insns[0].op, Opcode::Goto);
    assert_eq!(insns[1].op, Opcode::MoveResult);
    let v = violations
        .iter()
        .find(|v| {
            matches!(
                v,
                CodeItemInvariantViolation::BranchTargetIsMoveResultOrMoveException { .. }
            )
        })
        .expect("BONUS-3: BranchTargetIsMoveResultOrMoveException emitted");
    match v {
        CodeItemInvariantViolation::BranchTargetIsMoveResultOrMoveException {
            opcode,
            source_pc,
            target_pc,
            target_opcode,
        } => {
            assert_eq!(*opcode, Opcode::Goto);
            assert_eq!(*source_pc, 0);
            assert_eq!(*target_pc, 1);
            assert_eq!(*target_opcode, Opcode::MoveResult);
        }
        _ => unreachable!(),
    }
}

/// BONUS-4 — `packed-switch` pointing to an odd-address payload.
///
/// ART rejects via alignment check at `method_verifier.cc:2260-2266`.
/// DEX spec §6.4.2 requires payload address to be 32-bit aligned
/// (code-unit address must be even).
///
/// Layout:
///   pc=0 (3 units): PackedSwitch with offset=+1 → target=1 (odd address)
///   insns_size=3 (just the PackedSwitch; payload target is pc=1 which is
///   mid-instruction but also odd, triggering BONUS-4 specifically).
///
/// To isolate BONUS-4 cleanly, use a layout where the PackedSwitch offset
/// points to pc=5 (odd) which is past insns_size=3 so the main decode loop
/// doesn't try to enter a payload-skip branch there. The violation fires when
/// the switch instruction is decoded.
#[test]
fn probe_bonus4_unaligned_table_dex_pc() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    // PackedSwitch (0x2B, F31t, 3 code units):
    //   Unit 0: opcode=0x2B, AA=0x00
    //   Units 1+2: i32 offset = +5 → target = pc(0)+5 = 5 (odd address)
    // insns_size=3: only the PackedSwitch; payload target pc=5 is outside
    // insns_size so the skip-walker never enters the payload-skip branch
    // (the while loop exits at pc=3 after decoding the switch).
    let bytes: &[u8] = &[
        0x2B, 0x00, // op + AA=0
        0x05, 0x00, // offset low half = 5
        0x00, 0x00, // offset high half = 0 → offset=+5 → target=5 (odd)
    ];
    // insns_size=3: decode loop processes the PackedSwitch and exits.
    // BranchTargetOutOfRange fires (target=5 >= insns_size=3) AND
    // UnalignedTableDexPc fires (target=5 is odd). Both are expected.
    let (insns, payloads, violations) = decode_insns(bytes, 0, 3)
        .expect("BONUS-4: tolerant-parse retains unaligned payload reference");
    // The PackedSwitch instruction should be decoded.
    assert_eq!(insns.len(), 1, "BONUS-4: one PackedSwitch decoded");
    assert_eq!(insns[0].op, Opcode::PackedSwitch);
    // The payload is not in the map (target was unaligned → skip).
    assert!(
        !payloads.contains_key(&5),
        "BONUS-4: unaligned payload not in payloads map"
    );
    let v = violations
        .iter()
        .find(|v| matches!(v, CodeItemInvariantViolation::UnalignedTableDexPc { .. }))
        .expect("BONUS-4: UnalignedTableDexPc violation emitted");
    match v {
        CodeItemInvariantViolation::UnalignedTableDexPc {
            source_opcode,
            source_pc,
            payload_pc,
        } => {
            assert_eq!(*source_opcode, Opcode::PackedSwitch);
            assert_eq!(*source_pc, 0);
            assert_eq!(*payload_pc, 5, "BONUS-4: odd address payload_pc=5");
            assert_eq!(payload_pc % 2, 1, "BONUS-4: odd code-unit address confirmed");
        }
        _ => unreachable!(),
    }
}

/// BONUS-5 — last decoded instruction overshoots the declared `insns_size`.
///
/// ART's `ComputeWidthsAndCountOps` at `method_verifier.cc:1730-1801`
/// requires the decode loop to terminate exactly at `insns_size`. When the
/// last instruction's end address exceeds `insns_size`, the declared boundary
/// falls mid-instruction — ART rejects; droidsaw's loop exits without checking.
///
/// Layout:
///   insns_size=1: declares exactly 1 code unit.
///   At pc=0: `goto/16` (F20t, 2 code units, offset=+0 → target=0).
///   The instruction spans units 0..1, so its end address is pc=2 > insns_size=1.
///
/// Note: the loop condition `while pc < insns_size` lets pc=0 through; after
/// decoding goto/16 and advancing pc to 2, the loop exits. pc=2 > insns_size=1
/// triggers the violation.
#[test]
fn probe_bonus5_tail_bytes_after_last_instruction() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    // goto/16 (F20t, 0x29, 2 code units): offset in unit 1.
    // We need enough bytes so read_unit doesn't fail, but insns_size=1 declares
    // only 1 unit. The decoder reads the full 2-unit instruction starting at pc=0.
    let bytes: &[u8] = &[
        0x29, 0x00, // goto/16 opcode + AA=0
        0x00, 0x00, // unit 1: offset=0
    ];
    // insns_size=1: only 1 code unit declared; goto/16 needs 2 → overshoot.
    // The decoder at pc=0 reads both units (pc 0 and 1 are within `data`).
    // After advancing, pc=2 > insns_size=1 → TailBytesAfterLastInstruction.
    let (insns, _, violations) = decode_insns(bytes, 0, 1)
        .expect("BONUS-5: tolerant-parse retains overshot instruction");
    assert_eq!(insns.len(), 1, "BONUS-5: goto/16 decoded despite overshoot");
    assert_eq!(insns[0].op, Opcode::Goto16);
    let v = violations
        .iter()
        .find(|v| {
            matches!(v, CodeItemInvariantViolation::TailBytesAfterLastInstruction { .. })
        })
        .expect("BONUS-5: TailBytesAfterLastInstruction violation emitted");
    match v {
        CodeItemInvariantViolation::TailBytesAfterLastInstruction {
            insns_size,
            final_pc,
        } => {
            assert_eq!(*insns_size, 1, "BONUS-5: declared insns_size=1");
            assert_eq!(*final_pc, 2, "BONUS-5: goto/16 advance puts pc=2 > insns_size=1");
        }
        _ => unreachable!(),
    }
}

/// BONUS-6 — `invoke-virtual` with arg_count == 0 (missing receiver).
///
/// ART rejects via `method_verifier.cc:2047-2055` (kVerifyVarArgNonZero).
/// Non-static invokes require at least the receiver (`this`) argument.
///
/// Layout: F35c `invoke-virtual` with B nibble (arg_count) = 0.
///   Unit 0: opcode=0x6E (InvokeVirtual), B=0x0 (arg_count), A=0x0 (g-reg)
///   Unit 1: method_id = 0
///   Unit 2: regs C..F all zero
#[test]
fn probe_bonus6_non_static_invoke_arg_count_zero() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    // invoke-virtual (F35c, 0x6E):
    //   Unit 0 high nibble B (arg_count) = 0.
    //   Unit 0: 0x6E | (B=0x0 << 12) | (A=0x0 << 8) = 0x006E → LE [0x6E, 0x00]
    let bytes: &[u8] = &[
        0x6E, 0x00, // op + (B=0 arg_count | A=0 g-reg)
        0x00, 0x00, // method_id = 0
        0x00, 0x00, // C=0 D=0 E=0 F=0
    ];
    let (insns, _, violations) = decode_insns(bytes, 0, 3)
        .expect("BONUS-6: tolerant-parse retains zero-arg invoke");
    assert_eq!(insns.len(), 1);
    assert_eq!(insns[0].op, Opcode::InvokeVirtual);
    assert_eq!(insns[0].src.len(), 0, "BONUS-6: arg_count=0 → empty RegList");
    let v = violations
        .iter()
        .find(|v| {
            matches!(v, CodeItemInvariantViolation::NonStaticInvokeArgCountZero { .. })
        })
        .expect("BONUS-6: NonStaticInvokeArgCountZero violation emitted");
    match v {
        CodeItemInvariantViolation::NonStaticInvokeArgCountZero { opcode, source_pc } => {
            assert_eq!(*opcode, Opcode::InvokeVirtual);
            assert_eq!(*source_pc, 0);
        }
        _ => unreachable!(),
    }
}
