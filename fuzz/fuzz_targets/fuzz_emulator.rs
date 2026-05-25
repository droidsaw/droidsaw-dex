#![no_main]

//! Fuzz target for `EmulatorCore::execute`.
//!
//! Feeds arbitrary raw bytes as a Dalvik instruction stream by decoding
//! them through the existing `decode_insns` path and then executing via
//! `EmulatorCore`. The budget is capped at 1,000 instructions to keep
//! fuzz runs fast; the emulator enforces this budget and must not panic.
//!
//! Property: for all inputs, `execute` returns `Ok(value)` or a typed
//! `Err(EmulatorError::...)`. No panics, no stack overflows, no OOM.

use libfuzzer_sys::fuzz_target;
use droidsaw_dex::emulator::{self, EmulatorCore, Value};
use droidsaw_dex::decode::decode_insns;

fuzz_target!(|data: &[u8]| {
    // Need at least 3 bytes: registers_size (1 byte), ins_size (1 byte),
    // and at least 2 bytes of bytecode (one 16-bit code unit).
    if data.len() < 4 {
        return;
    }

    // First byte: register file size, clamped to [1, 32].
    let registers_size = ((data[0] & 0x1F) as u16).saturating_add(1);
    // Second byte: ins_size, clamped to [0, registers_size].
    let ins_size_raw = data[1] as u16;
    let ins_size = if registers_size == 0 {
        0
    } else {
        ins_size_raw % (registers_size + 1)
    };
    let bytecode = &data[2..];

    // The bytecode slice is treated as raw Dalvik 16-bit code units.
    // `decode_insns` interprets them as such. insns_size = number of code units.
    // Each code unit is 2 bytes, so code unit count = bytecode.len() / 2.
    let insns_size = (bytecode.len() / 2) as u32;
    if insns_size == 0 {
        return;
    }

    // Decode. If decoding fails, skip — the parser already handles
    // malformed bytes safely.
    let (instructions, payloads, _violations) = match decode_insns(bytecode, 0, insns_size) {
        Ok(r) => r,
        Err(_) => return,
    };

    if instructions.is_empty() {
        return;
    }

    let code_item = emulator::make_code_item_with_payloads(
        registers_size,
        ins_size,
        instructions,
        payloads,
    );

    // Argument tuple: all-zero ints of length ins_size.
    let args: Vec<Value> = (0..ins_size).map(|_| Value::Int(0)).collect();

    // Execute with a budget of 1,000.
    // Property: no panic. Any Ok or Err is valid.
    let _result = EmulatorCore::without_dex().execute(&code_item, &args, 1_000);
});
