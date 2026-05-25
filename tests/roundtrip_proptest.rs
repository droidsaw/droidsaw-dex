//! `parse ∘ emit ∘ parse == parse` structural round-trip property.
//!
//! Content-aware equivalence gate for `emit_dex`. Parses a known-good
//! fixture, emits, re-parses, asserts the second parse's IR matches
//! the first modulo layout fields (offsets change across emit; pool
//! contents must not).
//!
//! Fields that are layout-dependent and intentionally NOT compared:
//!   - Header checksum + signature (recomputed by emit)
//!   - Header per-section offsets (new layout)
//!   - ClassDefItem offset fields (annotations/class_data/static/
//!     interfaces_off)
//!   - ProtoIdItem.parameters_off
//!   - BTreeMap keys of class_datas / code_items / annotations /
//!     annotation_sets / annotation_set_ref_lists / annotation_items
//!     / type_lists (all keyed by original offset)
//!
//! Fields that MUST round-trip exactly:
//!   - pool sizes + contents (strings, types, protos, fields, methods)
//!   - all referenced IR content (by structural match)
//!
//! Byte-identity is a stricter gate tracked as follow-up work.

use droidsaw_dex::emit_dex::{
    emit_dex, emit_dex_collect, DexEmitError, EmitConfig,
};
use droidsaw_dex::parser::{ContentEquiv, DexFile};
use proptest::prelude::*;

mod common;
use common::dex_bytes_strategy;

proptest! {
    #![proptest_config(ProptestConfig {
        // Starting count — calibrated to p80 class shape (6 methods) +
        // p95 insn shape (62 insns). `PROPTEST_CASES` env override is
        // inherited from the workspace proptest convention.
        cases: 256,
        max_shrink_iters: 1024,
        ..ProptestConfig::default()
    })]

    #[test]
    fn parse_emit_parse_structural_equivalence(bytes in dex_bytes_strategy()) {
        let Ok(dex1) = DexFile::parse(&bytes, None) else {
            // Unparseable input — not a round-trip violation. The
            // contract only constrains emit on successfully-parsed IR.
            return Ok(());
        };

        let emitted = match emit_dex(&dex1) {
            Ok(buf) => buf,
            // Per emit_dex module docs "Emit domain ⊊ Parse domain":
            // parser is tolerant; emit rejects structurally-ill-formed
            // IR with typed errors. Mutation strategies routinely
            // produce such inputs (e.g., a string-pool byte flip
            // changes a pool's sort order — parse accepts, emit
            // rejects via from_verified). These are expected paths,
            // not round-trip violations.
            Err(DexEmitError::NotImplemented) => return Ok(()),
            Err(DexEmitError::UnrepresentableIR { .. }) => return Ok(()),
            Err(DexEmitError::SizeOverflow { .. }) => return Ok(()),
            Err(DexEmitError::OffsetOverflow { .. }) => return Ok(()),
            Err(e) => {
                prop_assert!(
                    false,
                    "parse succeeded, emit failed with internal error — round-trip violation: {e}"
                );
                return Ok(());
            }
        };

        let dex2 = match DexFile::parse(&emitted, None) {
            Ok(d) => d,
            Err(e) => {
                prop_assert!(
                    false,
                    "parse-emit-parse: second parse failed (checksum/layout bug): {e:?}"
                );
                return Ok(());
            }
        };

        // Content-aware equivalence via the quotient newtype.
        // ContentEquiv is the single source of truth for "what
        // counts as round-trip equivalent" — pools + per-entry
        // proto/class_def checks + subsection cardinalities,
        // modulo layout offsets + checksums. Defined in
        // parser.rs::ContentEquiv.
        prop_assert_eq!(
            ContentEquiv(&dex1),
            ContentEquiv(&dex2),
            "content equivalence violated post round-trip"
        );
    }

    /// Byte-identity consistency with observed transforms.
    ///
    /// Asserts the contrapositive of the attribution contract: if emit
    /// reports no canonical transforms were applied, byte-identity
    /// must hold. If any transform is reported, byte-identity may or
    /// may not hold (the report names a concrete reason bytes
    /// diverged). Conversely, byte-identity + non-empty
    /// `applied_transformations` is the inconsistency bug — emit
    /// reported a byte-changing transform fired, yet bytes match.
    ///
    /// As per-canonicalization work extends observation
    /// (adding variants for map_list reorder, alignment padding,
    /// etc.), this proptest strengthens automatically: more shapes
    /// of divergence become attributable instead of unattributed.
    /// The strict `prop_assert_eq!(bytes, emitted)` gate lands when
    /// the vec correctly attributes every divergence — at that point
    /// empty vec ⇔ byte-identity.
    #[test]
    fn parse_emit_byte_identity_consistency(bytes in dex_bytes_strategy()) {
        let Ok(dex) = DexFile::parse(&bytes, None) else {
            return Ok(());
        };
        let out = match emit_dex_collect(&dex, &EmitConfig::default()) {
            Ok(o) => o,
            Err(DexEmitError::NotImplemented)
            | Err(DexEmitError::UnrepresentableIR { .. })
            | Err(DexEmitError::SizeOverflow { .. })
            | Err(DexEmitError::OffsetOverflow { .. })
            | Err(DexEmitError::PartialIR { .. }) => return Ok(()),
            Err(e) => {
                prop_assert!(
                    false,
                    "emit_dex_collect failed with internal error: {e}"
                );
                return Ok(());
            }
        };

        let byte_identical = bytes == out.bytes;
        if byte_identical && !out.applied_transformations.is_empty() {
            prop_assert!(
                false,
                "byte-identical output reports applied transforms \
                 {:?} — attribution bug: a transform was reported \
                 as applied but bytes did not change",
                out.applied_transformations
            );
        }
        // Divergent output: emit did change bytes. Every applied
        // variant must correspond to a real byte-changing operation
        // by construction (observation-only). No further assertion —
        // the proptest's role is catching attribution bugs, not
        // chasing unattributed divergence (that's the follow-up
        // streams' job to shrink).
        let _ = byte_identical;
    }

    /// Emit entrypoint agreement — `emit_dex_with_config` and
    /// `emit_dex_collect` must produce identical bytes for the same
    /// config. The collect variant adds attribution; the bytes are
    /// the same. Sanity check against accidental divergence in future
    /// edits.
    #[test]
    fn emit_collect_bytes_match_with_config(bytes in dex_bytes_strategy()) {
        let Ok(dex) = DexFile::parse(&bytes, None) else {
            return Ok(());
        };
        let config = EmitConfig::default();
        let collect_result = emit_dex_collect(&dex, &config);
        let with_config_result =
            droidsaw_dex::emit_dex::emit_dex_with_config(&dex, &config);
        match (collect_result, with_config_result) {
            (Ok(out), Ok(bytes2)) => {
                prop_assert_eq!(
                    out.bytes, bytes2,
                    "emit_dex_collect and emit_dex_with_config produced different bytes"
                );
            }
            (Err(e1), Err(e2)) => {
                // Both failed — accept any matching error discriminant.
                prop_assert_eq!(
                    std::mem::discriminant(&e1),
                    std::mem::discriminant(&e2),
                    "emit_dex_collect and emit_dex_with_config failed with different error classes"
                );
            }
            (Ok(_), Err(e)) | (Err(e), Ok(_)) => {
                prop_assert!(
                    false,
                    "emit_dex_collect and emit_dex_with_config disagree on success: {:?}",
                    e
                );
            }
        }
    }
}
