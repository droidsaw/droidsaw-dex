#![no_main]

//! `fuzz_opcode_decode` — DEX instruction decoder structural-invariant gate.
//!
//! **Asserts (on any input where `decode_insns` succeeds):**
//! 1. No two instructions share the same `addr` (duplicate-offset
//!    invariant). Two instructions at the same address would make the
//!    instruction stream ambiguous for downstream CFG + SSA.
//! 2. Each instruction's `addr + size` does not overflow u32 (no
//!    wrap-around addresses in the decoded stream).
//! 3. Every instruction address is strictly less than `insns_size * 2`
//!    (the byte-stream length implied by `insns_size` u16 code units).
//!    An out-of-range address is a decoder bug.
//!
//! Panic-freedom invariant for instruction decoding plus structural
//! consistency checks.
//!
//! Note: full "encode/decode roundtrip" requires the opcode re-encoder.
//! These structural checks verify the decoding properties independently.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let insns_size = (data.len() / 2) as u32;
    let Ok((insns, _payloads, _violations)) =
        droidsaw_dex::decode::decode_insns(data, 0, insns_size)
    else {
        return;
    };

    // Inv 1: no duplicate addresses.
    let mut seen_addrs = std::collections::BTreeSet::new();
    for insn in &insns {
        assert!(
            seen_addrs.insert(insn.addr),
            "duplicate instruction address 0x{:x} in decoded stream",
            insn.addr,
        );
    }

    // Inv 2: addr + size does not overflow u32.
    for insn in &insns {
        let addr_end = insn.addr.checked_add(insn.size as u32);
        assert!(
            addr_end.is_some(),
            "instruction at addr 0x{:x} with size {} overflows u32",
            insn.addr,
            insn.size,
        );
    }

    // Inv 3: every address is within the declared instruction stream.
    // `insns_size` is in u16 code units; byte length = insns_size * 2.
    let byte_len = insns_size.saturating_mul(2);
    for insn in &insns {
        assert!(
            insn.addr < byte_len,
            "instruction addr 0x{:x} >= byte_len 0x{:x} (insns_size={})",
            insn.addr,
            byte_len,
            insns_size,
        );
    }
});
