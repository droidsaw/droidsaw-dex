#![no_main]

//! `fuzz_proguard_mapping` — R8/Proguard mapping-file parser panic surface.
//!
//! The R8 oracle ratchet at `tests/r8_oracle_ratchet.rs` feeds R8
//! mapping files into the `proguard` crate's
//! [`proguard::ProguardMapping`] iterator. The ratchet's panic
//! discipline lives in its OWN wrapper code (size cap, symlink
//! reject, frame-bounded extraction); it does NOT defend against
//! panics inside the upstream parser. This fuzz target drives the
//! upstream iterator on arbitrary bytes and asserts no panic.
//!
//! **Invariant.** For any byte slice `data`,
//! `ProguardMapping::new(data).iter().for_each(drop)` does not
//! panic. The iterator returns `Result<ProguardRecord, ParseError>`
//! per line; bad records are reported via `Err`, not via panic.
//!
//! A crash here is upstream's surface: file an issue against
//! `getsentry/rust-proguard` with the reproducer. droidsaw's
//! wrapper response is to pin the `proguard` crate's exact version
//! (already done at `=5.10.3`) until the upstream fix lands.

use libfuzzer_sys::fuzz_target;
use proguard::ProguardMapping;

fuzz_target!(|data: &[u8]| {
    // Cap input size at the same 64 MiB bound the ratchet enforces.
    // Beyond that, the test-time consumer refuses the read entirely
    // — fuzzing the iterator at multi-GB scale tests no production
    // path. The bound is generous; real R8 output on large APKs
    // lands in the single-digit-MiB range.
    if data.len() > 64 * 1024 * 1024 {
        return;
    }
    // Iterate every record. The iterator yields
    // `Result<ProguardRecord, ParseError>`; both arms are values
    // (no `?` short-circuit) so iteration drives the parser through
    // every line. `drop` discards the record content — we are
    // asserting non-panic, not content correctness.
    for record in ProguardMapping::new(data).iter() {
        drop(record);
    }
});
