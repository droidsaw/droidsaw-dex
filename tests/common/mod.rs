//! Shared scaffolding for tests under `droidsaw-dex/tests/`.
//!
//! Cargo's `tests/<dir>/mod.rs` convention compiles this file into
//! each consumer test binary rather than into a standalone test
//! target. Consumers add `mod common;` and use the items they need.
//!
//! Each consumer compiles its own copy of this module, and clippy
//! lints dead code per-binary; items used only by some consumers
//! appear unused in others. The crate-internal allow below avoids
//! that friction without hiding genuine dead code at the project
//! level.

#![allow(dead_code)]
//!
//! Currently hosts:
//! - `dex_bytes_strategy` (byte-mutation generator over the minimal
//!   multi-class fixture) for `roundtrip_proptest.rs` +
//!   `quotient_laws_proptest.rs`.
//! - [`r8_canonical_marker`] — strict-shape parser for
//!   `/* @droidsaw R8Origin(...) */` markers; shared by the
//!   production-corpus smoke test and the mapping-paired ratchet.
//! - [`r8_mapping_outline`] — outline-annotation parser for mapping.txt
//!   files, sibling to the synthesized-annotation parser in
//!   `r8_oracle_ratchet.rs`.

pub mod r8_canonical_marker;
pub mod r8_mapping_outline;

use proptest::prelude::*;

/// The minimal-fixture multi-class DEX used as the base for byte-
/// mutation strategies. Stable across roundtrip + laws proptests.
pub const FIXTURE: &[u8] = include_bytes!("../fixtures/classes.dex");

/// Byte-mutation strategy: identity / bit-flip / byte-substitution /
/// truncation. Calibrated against the D2 corpus audit (see comments
/// in `roundtrip_proptest.rs` history). Reused unchanged here so the
/// quotient-laws proptests probe the same input distribution as the
/// roundtrip proptest — the laws and the preservation invariant are
/// being checked on the same `parse(bytes) -> DexFile` shape.
pub fn dex_bytes_strategy() -> impl Strategy<Value = Vec<u8>> {
    let identity = Just(FIXTURE.to_vec()).boxed();

    let bit_flip = (0..FIXTURE.len(), 0u8..8)
        .prop_map(|(pos, bit)| {
            let mut out = FIXTURE.to_vec();
            out[pos] ^= 1 << bit;
            out
        })
        .boxed();

    let byte_subst = (0..FIXTURE.len(), any::<u8>())
        .prop_map(|(pos, val)| {
            let mut out = FIXTURE.to_vec();
            out[pos] = val;
            out
        })
        .boxed();

    let truncate = (32..FIXTURE.len())
        .prop_map(|len| FIXTURE[..len].to_vec())
        .boxed();

    prop_oneof![
        1 => identity,
        3 => bit_flip,
        2 => byte_subst,
        1 => truncate,
    ]
}
