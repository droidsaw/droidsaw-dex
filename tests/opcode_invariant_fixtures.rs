//! Binary-fixture regression suite for the opcode-invariant matrix fixes.
//!
//! Each `.bin` file under
//! `tests/fixtures/adversarial/opcode_invariant/` is a minimal raw-
//! instruction byte sequence that triggers the corresponding finding
//! in `decode::decode_insns`. The same files are seeded into
//! `fuzz/seeds/fuzz_opcode_decode/` so the libFuzzer corpus contains
//! the worked examples by default.
//!
//! Coverage (5 adversarial fixtures, one per fix class):
//!
//! | Fixture | Finding |
//! |---|---|
//! | `h1_invoke_arg_count_15.bin` | H-1 (F35c arg_count > 5) |
//! | `m1_goto32_target_oob.bin` | M-1 (source-instruction branch OOB) |
//! | `m1_packed_switch_case_target_oob.bin` | M-1 (payload-internal case-target OOB) |
//! | `m2_packed_switch_ident_mismatch.bin` | M-2 (payload ident mismatch) |
//! | `unchecked1_unknown_opcode_0x73.bin` | UNCHECKED-1 (unmapped opcode byte) |

use droidsaw_dex::decode::{decode_insns, CodeItemInvariantViolation};

const FX_H1: &[u8] = include_bytes!("fixtures/adversarial/opcode_invariant/h1_invoke_arg_count_15.bin");
const FX_M1_GOTO: &[u8] = include_bytes!("fixtures/adversarial/opcode_invariant/m1_goto32_target_oob.bin");
const FX_M1_SWITCH_CASE: &[u8] = include_bytes!(
    "fixtures/adversarial/opcode_invariant/m1_packed_switch_case_target_oob.bin"
);
const FX_M2: &[u8] = include_bytes!(
    "fixtures/adversarial/opcode_invariant/m2_packed_switch_ident_mismatch.bin"
);
const FX_UNCHECKED1: &[u8] = include_bytes!(
    "fixtures/adversarial/opcode_invariant/unchecked1_unknown_opcode_0x73.bin"
);

#[test]
fn fixture_h1_surfaces_opcode_arg_count_out_of_range() {
    let insns_size = (FX_H1.len() / 2) as u32;
    let (_, _, violations) = decode_insns(FX_H1, 0, insns_size).expect("H-1 fixture decodes");
    let count = violations
        .iter()
        .filter(|v| matches!(v, CodeItemInvariantViolation::OpcodeArgCountOutOfRange { .. }))
        .count();
    assert_eq!(count, 1, "H-1 fixture must produce one OpcodeArgCountOutOfRange violation");
}

#[test]
fn fixture_m1_goto_surfaces_branch_target_out_of_range() {
    let insns_size = (FX_M1_GOTO.len() / 2) as u32;
    let (_, _, violations) = decode_insns(FX_M1_GOTO, 0, insns_size).expect("M-1 goto fixture decodes");
    let count = violations
        .iter()
        .filter(|v| matches!(v, CodeItemInvariantViolation::BranchTargetOutOfRange { .. }))
        .count();
    assert_eq!(count, 1, "M-1 goto fixture must produce one BranchTargetOutOfRange violation");
}

#[test]
fn fixture_m1_packed_switch_case_surfaces_branch_target_out_of_range() {
    let insns_size = (FX_M1_SWITCH_CASE.len() / 2) as u32;
    let (_, _, violations) =
        decode_insns(FX_M1_SWITCH_CASE, 0, insns_size).expect("M-1 packed-switch case fixture decodes");
    // The fixture has one packed-switch case whose target is 0x7FFFFFFF.
    // The check fires for the payload-internal case-target (via the
    // resolve-loop payload-iteration check at decode.rs::decode_insns).
    let count = violations
        .iter()
        .filter(|v| matches!(v, CodeItemInvariantViolation::BranchTargetOutOfRange { .. }))
        .count();
    assert!(
        count >= 1,
        "M-1 packed-switch case fixture must produce at least one BranchTargetOutOfRange violation"
    );
}

#[test]
fn fixture_m2_surfaces_payload_ident_mismatch() {
    let insns_size = (FX_M2.len() / 2) as u32;
    let (_, _, violations) = decode_insns(FX_M2, 0, insns_size).expect("M-2 fixture decodes");
    let count = violations
        .iter()
        .filter(|v| matches!(v, CodeItemInvariantViolation::PayloadIdentMismatch { .. }))
        .count();
    assert_eq!(count, 1, "M-2 fixture must produce one PayloadIdentMismatch violation");
}

#[test]
fn fixture_unchecked1_surfaces_unknown_opcode_byte() {
    let insns_size = (FX_UNCHECKED1.len() / 2) as u32;
    let (_, _, violations) =
        decode_insns(FX_UNCHECKED1, 0, insns_size).expect("UNCHECKED-1 fixture decodes");
    let count = violations
        .iter()
        .filter(|v| matches!(v, CodeItemInvariantViolation::UnknownOpcodeByte { .. }))
        .count();
    assert_eq!(count, 1, "UNCHECKED-1 fixture must produce one UnknownOpcodeByte violation");
}
