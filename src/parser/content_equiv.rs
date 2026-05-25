//! Round-trip content-equivalence wrapper for `DexFile`. The PartialEq
//! impl is the single source of truth for "what counts as equivalent"
//! across `parse → emit → parse` roundtrips: pool content + subsection
//! cardinalities (ignoring layout-dependent fields like header offsets,
//! checksums, and BTreeMap keys keyed by on-disk offset).

use super::DexFile;

/// Content-equivalence wrapper for `DexFile`. Encodes the round-trip
/// contract `parse ∘ emit ∘ parse == parse` as a `PartialEq` impl
/// that compares semantic pool contents + subsection cardinalities,
/// ignoring layout-dependent fields (header offsets, checksums,
/// subsection BTreeMap keys which are keyed by on-disk offset and
/// legitimately differ across emit).
///
/// Previously this comparison was duplicated (manually, one field at
/// a time) across four test sites: `tests/roundtrip_proptest.rs`,
/// `tests/fixture_strip_emit.rs`, `tests/corpus_emit_smoke.rs`, and
/// `fuzz/fuzz_targets/fuzz_emit_roundtrip.rs`. Adding a subsection to
/// `DexFile` meant remembering to update four sites; drift risk.
///
/// Wrap a `&DexFile` and compare via this type; the `PartialEq` impl
/// is the single source of truth for "what counts as equivalent."
///
/// ## What is compared
///
/// - `strings`, `type_descriptors`, `fields`, `methods` — pool
///   contents, positional equality
/// - `class_defs.len()` — class count (individual `ClassDefItem`s
///   carry layout offsets that legitimately differ across emit;
///   content comparison at this level would require a layout-aware
///   field-by-field match)
/// - Subsection BTreeMap lengths — content equivalence modulo
///   offset-keyed map keys
///
/// ## What is NOT compared
///
/// - `header` — checksum + signature + section offsets, all
///   layout-dependent
/// - BTreeMap keys within each subsection map — original on-disk
///   offsets that emit rewrites
/// - Per-`ClassDefItem` offset fields (annotations_off etc)
///
/// For deeper content-level equivalence (walking inside each
/// subsection to compare values), extend with a separate
/// `DeepContentEquiv` wrapper. This level is the structural gate;
/// deep comparison is per-section and carries interpretive weight
/// (e.g., "are these two CodeItems semantically identical?" is a
/// richer question than structural eq allows).
#[derive(Debug)]
pub struct ContentEquiv<'a>(pub &'a DexFile);

impl<'a> PartialEq for ContentEquiv<'a> {
    fn eq(&self, other: &Self) -> bool {
        let a = self.0;
        let b = other.0;

        // Pool content — exact equality (no layout fields).
        if a.strings != b.strings
            || a.type_descriptors != b.type_descriptors
            || a.fields != b.fields
            || a.methods != b.methods
        {
            return false;
        }

        // Protos: per-entry equality modulo parameters_off (which is
        // a layout offset; presence bit is invariant).
        if a.protos.len() != b.protos.len() {
            return false;
        }
        for (pa, pb) in a.protos.iter().zip(b.protos.iter()) {
            if pa.shorty_idx != pb.shorty_idx
                || pa.return_type_idx != pb.return_type_idx
                || (pa.parameters_off == 0) != (pb.parameters_off == 0)
            {
                return false;
            }
        }

        // Class defs: per-entry equality modulo offset fields
        // (interfaces_off / annotations_off / class_data_off /
        // static_values_off are layout-dependent; their presence
        // bits are invariant).
        if a.class_defs.len() != b.class_defs.len() {
            return false;
        }
        for (ca, cb) in a.class_defs.iter().zip(b.class_defs.iter()) {
            if ca.class_idx != cb.class_idx
                || ca.access_flags != cb.access_flags
                || ca.superclass_idx != cb.superclass_idx
                || ca.source_file_idx != cb.source_file_idx
                || (ca.interfaces_off == 0) != (cb.interfaces_off == 0)
                || (ca.annotations_off == 0) != (cb.annotations_off == 0)
                || (ca.class_data_off == 0) != (cb.class_data_off == 0)
                || (ca.static_values_off == 0) != (cb.static_values_off == 0)
            {
                return false;
            }
        }

        // Subsection BTreeMap lengths — content comparison modulo
        // offset-keyed map keys. Deeper per-value comparison is a
        // separate `DeepContentEquiv` concern.
        //
        // Equivalence covers two extensions:
        // - method_handles / call_site_ids — lambda metafactory
        //   reconstruction pool equivalence.
        // - debug_info_raw_bytes per-entry byte-identity — round-trip
        //   invariant is byte-exact on the section, not just
        //   count-equivalent. BTreeMap iteration is
        //   deterministic (sorted by key); keys legitimately differ
        //   across emit (offset remap) so values are compared
        //   positionally via zip.
        if a.type_lists.len() != b.type_lists.len()
            || a.class_datas.len() != b.class_datas.len()
            || a.code_items.len() != b.code_items.len()
            || a.annotations.len() != b.annotations.len()
            || a.annotation_items.len() != b.annotation_items.len()
            || a.annotation_sets.len() != b.annotation_sets.len()
            || a.annotation_set_ref_lists.len() != b.annotation_set_ref_lists.len()
            || a.encoded_arrays.len() != b.encoded_arrays.len()
            || a.method_handles != b.method_handles
            || a.call_site_ids.len() != b.call_site_ids.len()
            || a.debug_infos.len() != b.debug_infos.len()
            || a.debug_info_raw_bytes.len() != b.debug_info_raw_bytes.len()
        {
            return false;
        }

        // DELIBERATELY NOT compared: `parse_errors`. A strict
        // equality check here creates a false-positive:
        // `parse → emit → parse` does NOT preserve the
        // error-generating malformed bytes (emit serializes clean IR,
        // so the round-tripped DexFile has an empty parse_errors
        // while the original has populated ones). Including
        // parse_errors in ContentEquiv would cause every
        // malformed-subsection input to fail roundtrip by
        // construction, drowning fuzz / corpus-smoke signals in
        // predictable noise. `parse_errors` preservation is a
        // separate property (would belong on a future
        // `ParseErrorPreserving` trait if an emit path ever opts in
        // via `EmitConfig { permit_partial_ir }`); it is not
        // implied by "same content IR".

        // Byte-identity gate on the debug_info section. Every preserved
        // entry must round-trip byte-exact through emit; if any entry
        // diverges, either `scan_debug_info_bytes` or
        // `emit_debug_info_section` has a bug.
        for (va, vb) in a
            .debug_info_raw_bytes
            .values()
            .zip(b.debug_info_raw_bytes.values())
        {
            if va != vb {
                return false;
            }
        }

        true
    }
}
