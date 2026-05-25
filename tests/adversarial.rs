//! Adversarial fixtures promoted from libFuzzer crashers.
//!
//! Each fixture is a minimal byte sequence that previously panicked or OOM'd
//! the parser/CFG/SSA paths. These tests lock in that the failure is now a
//! typed `DexError` (no panic, no RSS blow-up).

use droidsaw_dex::cfg::Cfg;
use droidsaw_dex::decode::{self, parse_class_data, parse_code_item};
use droidsaw_dex::error::DexError;
use droidsaw_dex::ssa::SsaBody;
use droidsaw_dex::DexFile;

/// Run the same pipeline `fuzz_targets/fuzz_cfg.rs` drives: parse → for each
/// class_def → parse_class_data → for each method with code_off → parse_code_item
/// → Cfg::build. Returns the first `DexError` surfaced, or `Ok(())` if the
/// input walks end-to-end without error.
fn drive_cfg(data: &[u8]) -> Result<(), DexError> {
    let dex = DexFile::parse(data, None)?;
    for cd in &dex.class_defs {
        if cd.class_data_off == 0 {
            continue;
        }
        let class_data = parse_class_data(data, cd.class_data_off)?;
        for em in class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
        {
            if em.code_off == 0 {
                continue;
            }
            let code = parse_code_item(data, em.code_off)?;
            let _ = Cfg::build(&code);
        }
    }
    Ok(())
}

/// Drives the `fuzz_ssa` pipeline: same as `drive_cfg` plus `SsaBody::build`.
fn drive_ssa(data: &[u8]) -> Result<(), DexError> {
    let dex = DexFile::parse(data, None)?;
    for cd in &dex.class_defs {
        if cd.class_data_off == 0 {
            continue;
        }
        let class_data = parse_class_data(data, cd.class_data_off)?;
        for em in class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
        {
            if em.code_off == 0 {
                continue;
            }
            let code = parse_code_item(data, em.code_off)?;
            let cfg = match Cfg::build(&code) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let _ = SsaBody::build(&code, &cfg);
        }
    }
    Ok(())
}

// Origin: fuzz/crashes/fuzz_parser/877d6398066a
// stage: parser
// panic: slice index starts at 12 but ends at 3
// site: src/header.rs:138:50
// Short input whose `file_size` header field is < 12, causing `&data[12..end]`
// to panic. Fix: header.rs guards `end < 12` and returns `DexError::Truncated`.
// The fixture also has an invalid header_size, which the layered
// `InvalidHeaderSize` gate now catches first; either variant satisfies the
// "no panic, typed Err" contract.
#[test]
fn fuzz_parser_877d6398066a_short_file_size() {
    let data = include_bytes!("fixtures/adversarial/fuzz_parser/877d6398066a.dex");
    let err = DexFile::parse(data, None).expect_err("must reject short-file_size input");
    assert!(
        matches!(
            err,
            DexError::Truncated { .. } | DexError::InvalidHeaderSize { .. }
        ),
        "expected Truncated or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_opcode_decode/2ca4d9928955
// stage: decode
// panic: attempt to add with overflow
// site: src/decode.rs:476:41
// bytes: 63 08 3f 3f fd 8f 11 04 ff ff
// F3rc `start_reg + i` overflowed when `start_reg` read from the stream was
// near u16::MAX. Fix: decode.rs uses `checked_add` and returns
// `DexError::InvalidInstruction`.
#[test]
fn fuzz_opcode_decode_2ca4d9928955_f3rc_start_reg_overflow() {
    let data = include_bytes!("fixtures/adversarial/fuzz_opcode_decode/2ca4d9928955.bin");
    let insns_size = (data.len() / 2) as u32;
    // decode_insns may succeed with Ok for inputs that decode cleanly; what
    // we require is no panic. This specific input hit the overflow branch
    // and must now return a typed Err.
    let result = decode::decode_insns(data, 0, insns_size);
    match result {
        Err(DexError::InvalidInstruction { .. }) => {}
        Err(other) => panic!("expected InvalidInstruction, got {other:?}"),
        Ok(_) => panic!("expected Err for overflow-triggering input, got Ok"),
    }
}

// Origin: fuzz/crashes/fuzz_opcode_decode/719b3a5e2b9b
// stage: decode
// panic: attempt to add with overflow
// site: src/decode.rs:280 (read_unit: pc + unit_idx)
// bytes: 2b 37 ff ff ff ff 25
// SparseSwitch branch offset near u32::MAX feeds a payload_pc that overflows
// when read_unit adds unit_idx. Fix: decode.rs uses `checked_add` and returns
// `DexError::InvalidInstruction`.
#[test]
fn fuzz_opcode_decode_719b3a5e2b9b_read_unit_pc_overflow() {
    let data = include_bytes!("fixtures/adversarial/fuzz_opcode_decode/719b3a5e2b9b.bin");
    let insns_size = (data.len() / 2) as u32;
    let result = decode::decode_insns(data, 0, insns_size);
    match result {
        Err(DexError::InvalidInstruction { .. }) => {}
        Err(other) => panic!("expected InvalidInstruction, got {other:?}"),
        Ok(_) => panic!("expected Err for overflow-triggering input, got Ok"),
    }
}

// Origin: fuzz/crashes/fuzz_opcode_decode/2de91997b6d1
// stage: decode
// panic: attempt to multiply with overflow
// site: src/decode.rs:690 (fill-array-data payload: size * width)
// bytes: 00 03 01 08 00 00 00 10 00 00 1b
// fill-array-data payload with stream-provided `size` and `width` whose u32
// product overflows. Fix: decode.rs uses `safe_mul_u32` and returns
// `DexError::ArithmeticOverflow`.
#[test]
fn fuzz_opcode_decode_2de91997b6d1_fill_array_data_size_width_overflow() {
    let data = include_bytes!("fixtures/adversarial/fuzz_opcode_decode/2de91997b6d1.bin");
    let insns_size = (data.len() / 2) as u32;
    let result = decode::decode_insns(data, 0, insns_size);
    match result {
        Err(DexError::ArithmeticOverflow { .. }) => {}
        Err(other) => panic!("expected ArithmeticOverflow, got {other:?}"),
        Ok(_) => panic!("expected Err for overflow-triggering input, got Ok"),
    }
}

// Origin: fuzz/artifacts/fuzz_opcode_decode/crash-651f454a2eb0… (60s smoke)
// stage: decode
// panic: attempt to add with overflow
// site: src/decode.rs:442 (F31t: `pc as i32 + offset` signed overflow)
// bytes: 2c 00 00 00 00 00 00 26 2c 22 ff ff ff 7f ff 28 c7 ff
// Chained SparseSwitch + FillArrayData whose 32-bit branch offset is
// 0x7fff_ffff class, so `pc as i32 + offset` exceeds i32::MAX. Fix:
// introduce `branch_target` helper using `u32::checked_add_signed` across
// all six F{10,20,21,22,30,31}t sites + both switch payload resolvers.
// Downstream the valid-but-astronomic target lands in payload resolution
// and falls out as ScrollRead — either typed error is acceptable, just
// not a panic.
#[test]
fn fuzz_opcode_decode_651f454a2eb0_f31t_branch_target_overflow() {
    let data = include_bytes!("fixtures/adversarial/fuzz_opcode_decode/651f454a2eb0.bin");
    let insns_size = (data.len() / 2) as u32;
    // Contract per the comment above: no panic on overflow-triggering input.
    // `Ok` is acceptable if the `checked_add_signed` discipline lets the
    // walker land on a valid target it can decode cleanly; `Err` is also
    // acceptable if the astronomic target lands in payload resolution.
    let result = decode::decode_insns(data, 0, insns_size);
    match result {
        Err(DexError::InvalidInstruction { .. }) | Err(DexError::ScrollRead { .. }) | Ok(_) => {}
        Err(other) => panic!("expected InvalidInstruction, ScrollRead, or Ok, got {other:?}"),
    }
}

// ── OOM-class crashers (amplification defense) ─────────────────────
//
// Each seed drives a parser/CFG/SSA path that would otherwise allocate
// multi-GiB from a tiny input because a u32 count field read from bytes
// flows straight into `Vec::with_capacity`. The resource-limits guard
// bounds every such site via `bound_count` →
// `DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { .. }`.
// These tests lock in the contract: no panic, no OOM, typed Err at the
// first bounded site that rejects the oversized count.

// Origin: fuzz/crashes/fuzz_parser/oom-c600defc01c6527d.bin
// stage: parser (amplification)
// kind: OOM — 112 B input drove multi-GB allocation
// site: parse_strings string_ids_size
// Fix: parser.rs bounds string_ids_size at data.len() / 4.
#[test]
fn fuzz_parser_oom_c600defc01c6527d_string_ids_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_parser/c600defc01c6527d.bin");
    let err = DexFile::parse(data, None).expect_err("must reject oversized string_ids_size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "string_ids", .. }) | DexError::InvalidHeaderSize { .. }),
        "expected CountExceedsInput(string_ids) or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_parser/oom-c0cb23eb4eaa
// stage: parser (amplification)
// kind: OOM — 3 KiB input drove a single 31 GiB allocation
// site: unbounded id-table pre-allocation inside DexFile::parse
// Fix: every id-table count in parser.rs (string/type/proto/field/method/
// class_defs) is now clamped against `data.len() / stride`. The input's
// counts now fit under the stride-bounded cap, so parse completes Ok —
// the allocation is no longer unbounded. Contract: completes without
// OOM / panic (the specific success/failure outcome is incidental).
#[test]
fn fuzz_parser_oom_c0cb23eb4eaa_amplification_no_oom() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_parser/c0cb23eb4eaa.bin");
    let _ = DexFile::parse(data, None);
}

// Origin: fuzz/crashes/fuzz_cfg/oom-0785eb2ecbb3084e.bin
// stage: parser (amplification — canonical)
// kind: OOM — 131 B input drove ~170.9 GiB allocation
// site: parse_class_defs class_defs_size
// Fix: parser.rs bounds class_defs_size at data.len() / 32.
#[test]
fn fuzz_cfg_oom_0785eb2ecbb3084e_class_defs_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_cfg/0785eb2ecbb3084e.bin");
    let err = drive_cfg(data).expect_err("must reject oversized class_defs_size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "class_defs", .. }) | DexError::InvalidHeaderSize { .. }),
        "expected CountExceedsInput(class_defs) or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_cfg/oom-268ac1d3377b
// stage: parser (amplification)
// site: parse_strings string_ids_size — rejected before reaching CFG.
#[test]
fn fuzz_cfg_oom_268ac1d3377b_string_ids_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_cfg/268ac1d3377b.bin");
    let err = drive_cfg(data).expect_err("must reject oversized string_ids_size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "string_ids", .. }) | DexError::InvalidHeaderSize { .. }),
        "expected CountExceedsInput(string_ids) or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_cfg/oom-291ceccdd32b
// stage: parser (amplification)
// site: parse_strings string_ids_size.
#[test]
fn fuzz_cfg_oom_291ceccdd32b_string_ids_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_cfg/291ceccdd32b.bin");
    let err = drive_cfg(data).expect_err("must reject oversized string_ids_size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "string_ids", .. }) | DexError::InvalidHeaderSize { .. }),
        "expected CountExceedsInput(string_ids) or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_cfg/oom-3178e3afbbe98018.bin
// stage: parser (amplification)
// site: parse_strings string_ids_size.
#[test]
fn fuzz_cfg_oom_3178e3afbbe98018_string_ids_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_cfg/3178e3afbbe98018.bin");
    let err = drive_cfg(data).expect_err("must reject oversized string_ids_size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "string_ids", .. }) | DexError::InvalidHeaderSize { .. }),
        "expected CountExceedsInput(string_ids) or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_cfg/oom-dc97e181aad5ff60.bin
// stage: parser (amplification)
// site: parse_strings string_ids_size.
#[test]
fn fuzz_cfg_oom_dc97e181aad5ff60_string_ids_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_cfg/dc97e181aad5ff60.bin");
    let err = drive_cfg(data).expect_err("must reject oversized string_ids_size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "string_ids", .. }) | DexError::InvalidHeaderSize { .. }),
        "expected CountExceedsInput(string_ids) or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_ssa/oom-0a8fa0997bb7cdf2.bin
// stage: parser (amplification)
// site: parse_class_defs class_defs_size — rejected before SSA.
#[test]
fn fuzz_ssa_oom_0a8fa0997bb7cdf2_class_defs_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_ssa/0a8fa0997bb7cdf2.bin");
    let err = drive_ssa(data).expect_err("must reject oversized class_defs_size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "class_defs", .. }) | DexError::InvalidHeaderSize { .. }),
        "expected CountExceedsInput(class_defs) or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_ssa/oom-4c840fa91be24db3.bin
// stage: parser (amplification)
// site: parse_strings string_ids_size.
#[test]
fn fuzz_ssa_oom_4c840fa91be24db3_string_ids_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_ssa/4c840fa91be24db3.bin");
    let err = drive_ssa(data).expect_err("must reject oversized string_ids_size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "string_ids", .. }) | DexError::InvalidHeaderSize { .. }),
        "expected CountExceedsInput(string_ids) or InvalidHeaderSize, got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_ssa/oom-6772da4a3bbd
// stage: parser (amplification)
// site: parse_type_lists size (inline count driving params Vec).
#[test]
fn fuzz_ssa_oom_6772da4a3bbd_type_list_amplification() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_ssa/6772da4a3bbd.bin");
    let err = drive_ssa(data).expect_err("must reject oversized type_list size");
    assert!(
        matches!(err, DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { item: "type_list", .. })),
        "expected CountExceedsInput(type_list), got {err:?}"
    );
}

// Origin: fuzz/crashes/fuzz_cfg/panic-437a5b34bb53.bin (15-min fuzz_cfg
//   gate, ~2.29M runs in)
// stage: decode (panic-class)
// panic: attempt to add with overflow
// site: src/decode.rs:1038 — `parse_class_data` `method_idx_acc += diff` in
//   the direct_methods loop
// Fix: converted all four `{field,method}_idx_acc += diff` sites in
//   `parse_class_data` (static_fields / instance_fields / direct_methods /
//   virtual_methods) to `checked_add` → DexError::InvalidInstruction with a
//   site-specific detail string.
//
// The layered access_flags spec-union gate now catches this fixture's
// invalid Field-scope access_flags first; either typed Err satisfies the
// "no panic" contract.
#[test]
fn fuzz_cfg_panic_437a5b34bb53_direct_methods_idx_acc_overflow() {
    let data = include_bytes!("fixtures/adversarial/fuzz_cfg/panic-437a5b34bb53-direct_methods.bin");
    // The crasher rides through DexFile::parse into the fuzz_cfg pipeline and
    // fires inside parse_class_data. The contract is: no panic, typed Err.
    let err = drive_cfg(data).expect_err("must surface typed Err before panic");
    match err {
        DexError::InvalidInstruction { ref detail, .. }
            if detail.contains("method_idx_diff accumulator overflow") => {}
        DexError::InvalidAccessFlags { .. } => {}
        DexError::InvalidHeaderSize { .. } => {}
        ref other => panic!(
            "expected InvalidInstruction(method_idx_diff accumulator overflow), \
             InvalidAccessFlags, or InvalidHeaderSize, got {other:?}"
        ),
    }
}

// Origin: fuzz/crashes/fuzz_ssa/oom-5263e994fb0c
// stage: decode (post-parse amplification)
// kind: OOM — 1841 B input blew past 2 GiB RSS during SSA
// Parse completes (counts now bounded); the input then references a
// code_off that points past end-of-data. The typed Err surfaces from
// `parse_code_item` (ScrollRead) rather than a CountExceedsInput site —
// the original OOM came from downstream SSA paths that never even run now.
// Contract: typed Err, no OOM / panic.
#[test]
fn fuzz_ssa_oom_5263e994fb0c_code_item_ooboffset() {
    let data = include_bytes!("fixtures/adversarial/oom/fuzz_ssa/5263e994fb0c.bin");
    let err = drive_ssa(data).expect_err("must surface a typed Err before OOM");
    // ScrollRead (code_off points past end-of-data), CountExceedsInput
    // (bounded amplification site), InvalidAccessFlags (per-scope mask
    // gate fires on this fixture's access_flags), or InvalidHeaderSize
    // (header-shape gate) — any typed Err satisfies the "no OOM" contract.
    match err {
        DexError::ScrollRead { .. }
        | DexError::BoundCountExceeded(droidsaw_common::guard::CountExceeded { .. })
        | DexError::InvalidAccessFlags { .. }
        | DexError::InvalidHeaderSize { .. } => {}
        other => panic!("expected ScrollRead / CountExceedsInput / InvalidAccessFlags / InvalidHeaderSize, got {other:?}"),
    }
}

// ── Budget exhaustion ──────────────────────────────────────────────────

#[test]
fn parse_budgeted_rejects_oversized_input_with_memory_error() {
    use droidsaw_common::budget::{BudgetExhausted, BudgetKind, Budget};

    // 200-byte input, 50-byte budget. charge(200, 0, "dex-parse-input")
    // must fire BudgetExhausted(Memory) before any DEX parsing.
    let data = vec![0u8; 200];
    let mut budget = Budget {
        memory_bytes_remaining: 50,
        steps_remaining: usize::MAX,
        deadline: None,
    };
    let err = DexFile::parse(&data, Some(&mut budget))
        .expect_err("must reject input exceeding memory budget");
    match err {
        DexError::Budget(BudgetExhausted {
            kind: BudgetKind::Memory,
            context: "dex-parse-input",
        }) => {}
        other => panic!(
            "expected Budget(Memory, \"dex-parse-input\"), got {other:?}"
        ),
    }
}

#[test]
fn parse_budgeted_exact_limit_accepts_then_one_over_fails() {
    use droidsaw_common::budget::{BudgetExhausted, BudgetKind, Budget};

    let data = vec![0u8; 100];

    // Exact fit: budget == input size → charge succeeds, parse proceeds
    // (parse itself will fail on bad magic, but no Budget error).
    let mut budget_ok = Budget {
        memory_bytes_remaining: 100,
        steps_remaining: usize::MAX,
        deadline: None,
    };
    let result_ok = DexFile::parse(&data, Some(&mut budget_ok));
    assert!(
        !matches!(result_ok, Err(DexError::Budget(_))),
        "exact-fit budget must not fire BudgetExhausted, got {result_ok:?}"
    );

    // One byte over: budget == input size - 1 → must fire Budget(Memory).
    let mut budget_over = Budget {
        memory_bytes_remaining: 99,
        steps_remaining: usize::MAX,
        deadline: None,
    };
    let err = DexFile::parse(&data, Some(&mut budget_over))
        .expect_err("1-byte-over budget must fire BudgetExhausted");
    assert!(
        matches!(
            err,
            DexError::Budget(BudgetExhausted {
                kind: BudgetKind::Memory,
                ..
            })
        ),
        "expected Budget(Memory), got {err:?}"
    );
}

// Adversarial fixture: header_map_disagreement/string_ids_off_by_one.dex
//
// Built from `fuzz/seeds/fuzz_parser/classes.dex` (a minimal 1236-byte
// real DEX produced by d8) by mutating the `map_list` entry for
// `TYPE_STRING_ID_ITEM (0x0001)` from `size=23` to `size=24` and
// recomputing the Adler-32 checksum over [12..file_size]. The header's
// `string_ids_size` remains 23, so the parser uses the correct count
// and parsing succeeds; the cross-check observes `header=23 map=24`
// and emits a `DEX_HEADER_MAP_DISAGREEMENT` finding for the
// `string_ids` section.
//
// Asserts:
// - `DexFile::parse` returns `Ok` (tolerant parse non-negotiable).
// - `dex.parse_errors` is empty (no MapList unreadable signal).
// - `collect_header_map_findings(&dex)` returns exactly one Finding
//   with `id == DEX_HEADER_MAP_DISAGREEMENT`, detail
//   `section=string_ids header=23 map=24`, and extra `type_code=0x0001`.
// - `dex.strings.len() == 23` (parsing continued using the header-side
//   count, not the map_list-side count).
#[test]
fn header_map_disagreement_string_ids_off_by_one() {
    use droidsaw_dex::diag::{collect_header_map_findings, FINDING_ID_DEX_HEADER_MAP_DISAGREEMENT};

    let data =
        include_bytes!("fixtures/adversarial/header_map_disagreement/string_ids_off_by_one.dex");
    let dex = DexFile::parse(data, None).expect("adversarial fixture must parse Ok");

    assert!(
        dex.parse_errors.is_empty(),
        "parse_errors must be empty (map_list itself parsed cleanly); got {:?}",
        dex.parse_errors
    );

    let findings = collect_header_map_findings(&dex);
    assert_eq!(
        findings.len(),
        1,
        "expected exactly one disagreement finding; got {findings:?}"
    );
    let f = &findings[0];
    assert_eq!(f.id, FINDING_ID_DEX_HEADER_MAP_DISAGREEMENT);
    assert_eq!(f.detail, "section=string_ids header=23 map=24");
    assert_eq!(f.extra.as_deref(), Some("type_code=0x0001"));

    assert_eq!(
        dex.strings.len(),
        23,
        "string parsing must use header.string_ids_size (23), not map_list size (24); got {}",
        dex.strings.len()
    );
}

// Adversarial fixture: string_length_disagree/length_disagree.dex
//
// Built from `tests/fixtures/classes.dex` (the same 1236-byte real DEX
// used to seed the header_map_disagreement sibling fixture) by
// mutating string[0]'s ULEB128 declared char-count prefix from `0x06`
// to `0x07` at file offset `0x22e`, and recomputing SHA-1 + Adler-32.
// The string body is `<init>` (6 bytes, 6 UTF-16 units after MUTF-8
// decode); declared=7 ≠ scanned=6 is a per-record TARmageddon
// disagreement (CVE-2025-62518 §6 generalization).
//
// Asserts the fixture contract:
// - `DexFile::parse` returns `Ok` (tolerant parse non-negotiable).
// - The parser captured `declared_string_lengths[0] == 7` (the
//   ULEB128 value is preserved, not discarded).
// - `dex.strings[0] == "<init>"` (parser used NUL-scan-derived
//   position, preserving observable behavior on the body).
// - `collect_string_length_findings(&dex)` returns exactly one
//   `DEX_STRING_LENGTH_DISAGREEMENT` Finding (rolled-up, per design
//   note), with `detail` carrying the count + max-gap summary and
//   `extra` carrying an `idx=0 declared=7 scanned=6` sample row.
// - The fixture does NOT trip `DEX_STRING_MISSING_TERMINATOR` —
//   string[0]'s NUL terminator is still present in the body.
#[test]
fn string_length_disagree_init_off_by_one() {
    use droidsaw_dex::diag::{
        collect_string_length_findings, FINDING_ID_DEX_STRING_LENGTH_DISAGREEMENT,
        FINDING_ID_DEX_STRING_MISSING_TERMINATOR,
    };

    let data =
        include_bytes!("fixtures/adversarial/string_length_disagree/length_disagree.dex");
    let dex = DexFile::parse(data, None).expect("adversarial fixture must parse Ok");

    assert!(
        dex.parse_errors.is_empty(),
        "parse_errors must be empty; got {:?}",
        dex.parse_errors
    );
    assert_eq!(
        dex.strings.first().map(|s| s.declared_chars()),
        Some(7),
        "parser must capture the ULEB128 declared char-count (7), \
         not discard it; got {:?}",
        dex.strings.first().map(|s| s.declared_chars())
    );
    assert_eq!(
        dex.strings.first().map(|s| s.as_str_lossy()),
        Some("<init>"),
        "parser must use NUL-scan-derived position (yields `<init>`, \
         6 UTF-16 units), not the declared count"
    );

    let findings = collect_string_length_findings(&dex);
    let disagreements: Vec<&_> = findings
        .iter()
        .filter(|f| f.id == FINDING_ID_DEX_STRING_LENGTH_DISAGREEMENT)
        .collect();
    let missing_terminators: Vec<&_> = findings
        .iter()
        .filter(|f| f.id == FINDING_ID_DEX_STRING_MISSING_TERMINATOR)
        .collect();
    assert_eq!(
        disagreements.len(),
        1,
        "expected exactly one rolled-up disagreement Finding; \
         got {findings:?}"
    );
    assert!(
        missing_terminators.is_empty(),
        "string[0] has a valid NUL terminator; missing_terminator \
         must not fire on this fixture; got {missing_terminators:?}"
    );
    let f = disagreements[0];
    assert_eq!(
        f.detail,
        "1 string with declared/scanned UTF-16 length disagreement (max gap: 1 units)"
    );
    assert_eq!(f.extra.as_deref(), Some("idx=0 declared=7 scanned=6"));
}

// Adversarial fixture: string_length_disagree/missing_terminator.dex
//
// Built from `tests/fixtures/classes.dex` by appending `0x01 0x41`
// (ULEB128(1) + ASCII 'A') to the file with NO trailing `0x00`,
// updating `header.file_size` (offset 0x20) to 1238, re-pointing
// `string_id_item[0].string_data_off` to the new region (offset 1236),
// and recomputing SHA-1 + Adler-32. The NUL-scan in `parse_strings`
// for string[0] runs over `data[1237..1238]` = `b"A"` and finds no
// `0x00`, triggering the `unwrap_or(data.len())` extend-to-EOF path
// — the load-bearing missing-terminator signal.
//
// Asserts:
// - `DexFile::parse` returns `Ok` (tolerant parse non-negotiable).
// - `dex.missing_terminator_marks[0] == true` (parser captured the
//   silent extend-to-EOF; this is the pre-Finding gauge).
// - `collect_string_length_findings(&dex)` returns exactly one
//   `DEX_STRING_MISSING_TERMINATOR` Finding with `detail` naming
//   `string_idx=0` and `extra` carrying `idx=0` (per-string
//   emission, not rolled up — design note: corpus rate 0/985 makes
//   this rare-event-diagnostic).
// - The fixture does NOT trip `DEX_STRING_LENGTH_DISAGREEMENT` —
//   declared=1, parsed string `"A"` lossily decoded to 1 UTF-16
//   unit → declared == scanned → spec-permitted equivalence.
#[test]
fn string_missing_terminator_extends_to_eof() {
    use droidsaw_dex::diag::{
        collect_string_length_findings, FINDING_ID_DEX_STRING_LENGTH_DISAGREEMENT,
        FINDING_ID_DEX_STRING_MISSING_TERMINATOR,
    };

    let data =
        include_bytes!("fixtures/adversarial/string_length_disagree/missing_terminator.dex");
    let dex = DexFile::parse(data, None).expect("adversarial fixture must parse Ok");

    // NOTE: `parse_errors` is non-empty for this fixture by design — the
    // mutation that produces the missing-terminator condition (re-pointing
    // string[0]'s `string_data_off` to a region we appended past the
    // original `data_off`/`data_size` window) shifts the relative
    // positioning of downstream sections, so `code_item` /
    // `annotation_directory` parses encounter stale offsets and emit
    // `ParseFailure` records. None of those affect the string-pool
    // surface this fixture targets; the missing-terminator
    // assertions concern `missing_terminator_marks` + the
    // `DEX_STRING_MISSING_TERMINATOR` Finding only.
    assert_eq!(
        dex.strings.first().map(|s| !s.had_terminator()),
        Some(true),
        "parser must mark string[0] as missing-terminator (NUL-scan \
         hit `unwrap_or(data.len())` fallback); got had_terminator={:?}",
        dex.strings.first().map(|s| s.had_terminator())
    );

    let findings = collect_string_length_findings(&dex);
    let disagreements: Vec<&_> = findings
        .iter()
        .filter(|f| f.id == FINDING_ID_DEX_STRING_LENGTH_DISAGREEMENT)
        .collect();
    let missing_terminators: Vec<&_> = findings
        .iter()
        .filter(|f| f.id == FINDING_ID_DEX_STRING_MISSING_TERMINATOR)
        .collect();
    assert_eq!(
        missing_terminators.len(),
        1,
        "expected exactly one missing-terminator Finding for string[0]; \
         got {findings:?}"
    );
    assert!(
        disagreements.is_empty(),
        "declared=1 + lossy-decoded `\"A\"` (1 UTF-16 unit) → \
         declared==scanned → no disagreement should fire; got {disagreements:?}"
    );
    let f = missing_terminators[0];
    assert_eq!(
        f.detail,
        "string_idx=0 missing 0x00 terminator; parser extended-to-EOF"
    );
    assert_eq!(f.extra.as_deref(), Some("idx=0"));
}

// Adversarial fixture: try_item_range/try_start_addr_max.dex
//
// Sourced from a 7536-byte real DEX (a minimal proof-of-concept app)
// containing 21 methods of which one has a single try_item; the
// builder mutated that try_item's first u32 (`start_addr`) to
// `u32::MAX` and its u16 (`insn_count`) to `1`, then recomputed
// Adler-32. The seed was chosen as the smallest try-bearing DEX in
// the RE corpus (≤ 32KB cap) — the in-tree seeds at
// `tests/fixtures/*.dex` and `fuzz/seeds/*` have zero try-bearing
// methods (d8's minimal-class output omits exception handlers).
//
// The mutation produces:
//   - `start_addr = 0xFFFFFFFF` (= u32::MAX)
//   - `insn_count = 1` (originally a small benign value)
// so the natural `start_addr + insn_count as u32` wraps to 0 — the
// CFG silent-edge-drop primitive from `cfg.rs:439, :514`.
//
// Asserts:
// - `DexFile::parse` returns `Ok` (tolerant parse non-negotiable).
// - At least one code_item carries a
//   `CodeItemInvariantViolation::TryItemRangeInvalid` recording
//   `start_addr = u32::MAX` and `insn_count = 1`.
// - The parser CLAMPED the in-IR `insn_count` to
//   `insns_size.saturating_sub(start_addr) = 0`, so consumers using
//   `try.start_addr + try.insn_count as u32` see the clamp, not the
//   wrap (the load-bearing CFG-correctness gauge).
// - `collect_code_item_findings(&dex)` returns at least one
//   `DEX_TRY_ITEM_RANGE_INVALID` Finding with the expected
//   pre-clamp evidence in `detail` + `extra`.
#[test]
fn try_item_start_addr_max_clamps_and_emits_finding() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    use droidsaw_dex::diag::{
        collect_code_item_findings, FINDING_ID_DEX_TRY_ITEM_RANGE_INVALID,
    };

    let data = include_bytes!("fixtures/adversarial/try_item_range/try_start_addr_max.dex");
    let dex = DexFile::parse(data, None).expect("adversarial fixture must parse Ok");

    // Locate the violation record.
    let mut viol_code_off: Option<u32> = None;
    let mut viol_record: Option<(u16, u32, u16, u32)> = None;
    for (off, code) in &dex.code_items {
        for v in &code.invariant_violations {
            if let CodeItemInvariantViolation::TryItemRangeInvalid {
                try_idx,
                start_addr,
                insn_count,
                insns_size,
            } = *v
            {
                viol_code_off = Some(*off);
                viol_record = Some((try_idx, start_addr, insn_count, insns_size));
                break;
            }
        }
        if viol_code_off.is_some() {
            break;
        }
    }
    let (try_idx, start_addr, insn_count, insns_size) =
        viol_record.expect("at least one TryItemRangeInvalid violation must be recorded");
    let code_off = viol_code_off.expect("code_off of the offending code_item");

    assert_eq!(start_addr, u32::MAX, "pre-clamp start_addr preserved in violation record");
    assert_eq!(insn_count, 1, "pre-clamp insn_count preserved in violation record");
    // insns_size came from the original seed method; we don't pin
    // its exact value (varies with the source method) but it must be a
    // legitimate code-unit count < u32::MAX (otherwise the cross-check
    // wouldn't have fired).
    assert!(
        insns_size < u32::MAX,
        "insns_size must be a legitimate code-unit count, got {insns_size}"
    );

    // The in-IR clamp: insn_count clamped to
    // `insns_size.saturating_sub(start_addr)` which for start_addr =
    // u32::MAX is 0 (saturating). So the offending try's `insn_count`
    // is now `0`, NOT the original 1.
    let code = dex.code_items.get(&code_off).expect("code_item present");
    let clamped = code
        .tries
        .get(try_idx as usize)
        .expect("try_idx in bounds")
        .insn_count;
    assert_eq!(
        clamped, 0,
        "in-IR insn_count must be clamped to insns_size.saturating_sub(u32::MAX) = 0 \
         (the load-bearing CFG-correctness invariant); got {clamped}"
    );

    let findings = collect_code_item_findings(&dex);
    let try_findings: Vec<&_> = findings
        .iter()
        .filter(|f| f.id == FINDING_ID_DEX_TRY_ITEM_RANGE_INVALID)
        .collect();
    assert!(
        !try_findings.is_empty(),
        "expected ≥1 DEX_TRY_ITEM_RANGE_INVALID Finding; got {findings:?}"
    );
    let f = try_findings[0];
    // detail format: "try_idx={try_idx} start_addr={start_addr} insn_count={insn_count} insns_size={insns_size}"
    assert!(
        f.detail.contains(&format!("start_addr={}", u32::MAX)),
        "Finding detail must carry pre-clamp start_addr; got {:?}",
        f.detail
    );
    assert!(
        f.detail.contains("insn_count=1"),
        "Finding detail must carry pre-clamp insn_count; got {:?}",
        f.detail
    );
    assert_eq!(
        f.extra.as_deref().map(|s| s.contains("try_idx=")),
        Some(true),
        "Finding extra must carry try_idx; got {:?}",
        f.extra
    );
}

// Adversarial fixture: try_item_range/ins_exceeds_registers.dex
//
// Built from `tests/fixtures/classes.dex` by mutating the first
// code_item's `registers_size` u16 (at offset `code_off + 0`) from 2
// to 0, leaving `ins_size = 1`. The cross-check observes
// `ins_size > registers_size` and records
// `CodeItemInvariantViolation::RegisterCountInverted`. Downstream
// `ssa.rs:393`'s `registers_size.saturating_sub(ins_size)` would
// otherwise silently produce 0 → all parameters attributed to
// overlapping register slots → wrong SSA. No in-IR clamp is applied
// (downstream policy).
//
// Asserts:
// - `DexFile::parse` returns `Ok`.
// - The offending code_item carries
//   `CodeItemInvariantViolation::RegisterCountInverted { registers_size: 0, ins_size: 1 }`.
// - `collect_code_item_findings(&dex)` returns exactly one
//   `DEX_CODE_ITEM_REGISTER_COUNT_INVERTED` Finding with detail
//   carrying both observed values.
#[test]
fn code_item_register_count_inverted_emits_finding() {
    use droidsaw_dex::decode::CodeItemInvariantViolation;
    use droidsaw_dex::diag::{
        collect_code_item_findings, FINDING_ID_DEX_CODE_ITEM_REGISTER_COUNT_INVERTED,
    };

    let data = include_bytes!("fixtures/adversarial/try_item_range/ins_exceeds_registers.dex");
    let dex = DexFile::parse(data, None).expect("adversarial fixture must parse Ok");

    let mut viol: Option<(u16, u16)> = None;
    for code in dex.code_items.values() {
        for v in &code.invariant_violations {
            if let CodeItemInvariantViolation::RegisterCountInverted {
                registers_size,
                ins_size,
            } = *v
            {
                viol = Some((registers_size, ins_size));
            }
        }
    }
    let (registers_size, ins_size) =
        viol.expect("at least one RegisterCountInverted violation must be recorded");
    assert_eq!(registers_size, 0);
    assert_eq!(ins_size, 1);

    let findings = collect_code_item_findings(&dex);
    let inv_findings: Vec<&_> = findings
        .iter()
        .filter(|f| f.id == FINDING_ID_DEX_CODE_ITEM_REGISTER_COUNT_INVERTED)
        .collect();
    assert_eq!(
        inv_findings.len(),
        1,
        "expected exactly one DEX_CODE_ITEM_REGISTER_COUNT_INVERTED Finding; got {findings:?}"
    );
    let f = inv_findings[0];
    assert_eq!(f.detail, "registers_size=0 ins_size=1");
}

// Adversarial fixture: string_length_disagree/lone_surrogate.dex
//
// Real-world-representative: matches the dominant Phase-0 corpus
// shape (110/985 DEXes, ~5,016 per-string disagreements, scanned > declared
// in nearly every case). Built from `tests/fixtures/classes.dex` by
// appending the 7-byte string body `02 41 ED A0 80 42 00`:
//   - `02`        ULEB128 declared = 2 UTF-16 code units
//   - `41`        ASCII 'A'
//   - `ED A0 80`  Lone high surrogate (codepoint U+D800) — NO matching
//                 low surrogate sequence follows
//   - `42`        ASCII 'B'
//   - `00`        NUL terminator
// then re-pointing `string_id_item[0].string_data_off` to the new
// region, updating `header.file_size` (1236 → 1243), and recomputing
// Adler-32.
//
// `decode_mutf8` rejects at the surrogate-pair sub-state (`0xED 0xA0
// 0x80` is read as code 0xD800, in the high-surrogate range
// 0xD800-0xDBFF, but the bytes that follow are not the expected
// low-surrogate `0xED 0xB?-0xBF 0x??` shape — see
// `droidsaw-common/src/encoding.rs:210-227`). The parser falls back
// to `String::from_utf8_lossy`, which substitutes `U+FFFD` per
// invalid UTF-8 byte. Post-fallback decoded string is
// `"A\u{FFFD}\u{FFFD}\u{FFFD}B"` (5 UTF-16 units) versus declared 2,
// gap = -3 (matching the Phase-0 dominant direction).
//
// Asserts the load-bearing real-world-shape contract that the
// synthetic ULEB128-mutation fixture above does NOT cover:
// - `DexFile::parse` returns `Ok` (tolerant parse non-negotiable).
// - `dex.lossy_decode_marks[0] == true` (the gauge for "decoder
//   actually fell back" — the upstream signal that drives the
//   downstream UTF-16 count divergence).
// - `dex.declared_string_lengths[0] == 2` (ULEB128 prefix captured).
// - `dex.strings[0]` decodes to a 5-UTF-16-unit lossy form.
// - `collect_string_length_findings(&dex)` emits the expected
//   `DEX_STRING_LENGTH_DISAGREEMENT` rollup with detail `"1 string …
//   max gap: -3 units"` and `extra` `"idx=0 declared=2 scanned=5"`.
// - The fixture does NOT trip `DEX_STRING_MISSING_TERMINATOR` —
//   string[0]'s NUL terminator is the seventh appended byte.
#[test]
fn string_length_disagree_lone_surrogate_lossy_fallback() {
    use droidsaw_dex::diag::{
        collect_string_length_findings, FINDING_ID_DEX_STRING_LENGTH_DISAGREEMENT,
        FINDING_ID_DEX_STRING_MISSING_TERMINATOR,
    };

    let data = include_bytes!("fixtures/adversarial/string_length_disagree/lone_surrogate.dex");
    let dex = DexFile::parse(data, None).expect("adversarial fixture must parse Ok");

    assert!(
        dex.strings[0].is_lossy(),
        "lone surrogate must trip decode_mutf8 → MalformedMutf8 variant; \
         got is_lossy={}",
        dex.strings[0].is_lossy()
    );
    assert_eq!(
        dex.strings[0].declared_chars(), 2,
        "parser must capture the ULEB128 declared count (2); got {}",
        dex.strings[0].declared_chars()
    );
    assert_eq!(
        dex.strings[0].as_str_lossy().encode_utf16().count(),
        5,
        "lossy fallback substitutes U+FFFD per invalid byte; \
         A + 3 U+FFFD + B = 5 UTF-16 units; got {}",
        dex.strings[0].as_str_lossy().encode_utf16().count()
    );

    let findings = collect_string_length_findings(&dex);
    let disagreements: Vec<&_> = findings
        .iter()
        .filter(|f| f.id == FINDING_ID_DEX_STRING_LENGTH_DISAGREEMENT)
        .collect();
    let missing_terminators: Vec<&_> = findings
        .iter()
        .filter(|f| f.id == FINDING_ID_DEX_STRING_MISSING_TERMINATOR)
        .collect();
    assert!(
        missing_terminators.is_empty(),
        "NUL terminator is intact on this fixture; missing_terminator \
         must not fire; got {missing_terminators:?}"
    );
    assert_eq!(
        disagreements.len(),
        1,
        "exactly one rolled-up disagreement Finding expected; got {findings:?}"
    );
    let f = disagreements[0];
    assert_eq!(
        f.detail,
        "1 string with declared/scanned UTF-16 length disagreement (max gap: -3 units)"
    );
    assert_eq!(f.extra.as_deref(), Some("idx=0 declared=2 scanned=5"));
}
