//! Locate the first byte that differs between an input DEX and droidsaw's
//! parse-emit-parse output, and identify which `map_entries` section it
//! lives in. Validates (or refutes) the data-section subsection layout
//! reordering hypothesis from the v1 release artifact.
//!
//! Opt in:
//!
//! ```bash
//! DROIDSAW_FIRST_DIFF_INPUT_DEX=/tmp/poc-input.dex \
//!     cargo test --release --test first_diff_diagnostic -- --nocapture
//! ```
//!
//! Output: first byte position that differs, the section type it falls in
//! (per input's map_entries), context bytes around the divergence in both
//! input and output, plus a side-by-side section start table.

use droidsaw_dex::emit_dex::{emit_dex_collect, map_type, EmitConfig};
use droidsaw_dex::parser::{DexFile, MapEntry};

fn section_label(tc: u16) -> &'static str {
    match tc {
        x if x == map_type::HEADER_ITEM => "header",
        x if x == map_type::STRING_ID_ITEM => "string_ids",
        x if x == map_type::TYPE_ID_ITEM => "type_ids",
        x if x == map_type::PROTO_ID_ITEM => "proto_ids",
        x if x == map_type::FIELD_ID_ITEM => "field_ids",
        x if x == map_type::METHOD_ID_ITEM => "method_ids",
        x if x == map_type::CLASS_DEF_ITEM => "class_defs",
        x if x == map_type::CALL_SITE_ID_ITEM => "call_site_ids",
        x if x == map_type::METHOD_HANDLE_ITEM => "method_handles",
        x if x == map_type::STRING_DATA_ITEM => "string_data",
        x if x == map_type::TYPE_LIST => "type_list",
        x if x == map_type::CODE_ITEM => "code_item",
        x if x == map_type::CLASS_DATA_ITEM => "class_data",
        x if x == map_type::ANNOTATION_ITEM => "annotation",
        x if x == map_type::ANNOTATION_SET_ITEM => "annotation_set",
        x if x == map_type::ANNOTATION_SET_REF_LIST => "annotation_set_ref_list",
        x if x == map_type::ANNOTATION_DIRECTORY_ITEM => "annotation_directory",
        x if x == map_type::ENCODED_ARRAY_ITEM => "encoded_array",
        x if x == map_type::DEBUG_INFO_ITEM => "debug_info",
        x if x == map_type::MAP_LIST => "map_list",
        _ => "unknown",
    }
}

fn section_ranges_sorted(file_size: usize, entries: &[MapEntry]) -> Vec<(u16, usize, usize)> {
    let mut sorted: Vec<MapEntry> = entries.to_vec();
    sorted.sort_by_key(|e| e.offset);
    let mut out = Vec::new();
    for (i, e) in sorted.iter().enumerate() {
        let start = e.offset as usize;
        let end = sorted
            .get(i + 1)
            .map(|n| n.offset as usize)
            .unwrap_or(file_size);
        out.push((e.type_code, start.min(file_size), end.min(file_size)));
    }
    out
}

fn section_containing(ranges: &[(u16, usize, usize)], pos: usize) -> Option<(u16, usize, usize)> {
    ranges
        .iter()
        .find(|(_, s, e)| pos >= *s && pos < *e)
        .copied()
}

fn hex_dump(bytes: &[u8], start: usize, len: usize) -> String {
    let end = (start + len).min(bytes.len());
    bytes[start..end]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn first_diff_locator() {
    let Ok(path) = std::env::var("DROIDSAW_FIRST_DIFF_INPUT_DEX") else {
        eprintln!(
            "first_diff_locator: DROIDSAW_FIRST_DIFF_INPUT_DEX unset; skipping"
        );
        return;
    };
    let input_bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let dex_in = DexFile::parse(&input_bytes, None).expect("parse input dex");
    let cfg = EmitConfig {
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        preserve_data_section_layout: true,
        ..Default::default()
    };
    let out = emit_dex_collect(&dex_in, &cfg).expect("emit");
    let output_bytes = out.bytes;
    let dex_out = DexFile::parse(&output_bytes, None).expect("re-parse output");

    eprintln!("\n=== first_diff_locator: {path} ===");
    eprintln!(
        "input_len={} output_len={}",
        input_bytes.len(),
        output_bytes.len()
    );

    // Side-by-side section starts table, sorted by INPUT offset.
    eprintln!("\n--- section start comparison (input vs output, by type_code) ---");
    let in_ranges = section_ranges_sorted(input_bytes.len(), &dex_in.map_entries);
    let out_ranges = section_ranges_sorted(output_bytes.len(), &dex_out.map_entries);
    let out_by_tc: std::collections::HashMap<u16, (usize, usize)> = out_ranges
        .iter()
        .map(|(tc, s, e)| (*tc, (*s, *e)))
        .collect();
    let mut subsections_in_order_input: Vec<&str> = Vec::new();
    let mut subsections_in_order_output: Vec<(u16, usize)> = out_ranges
        .iter()
        .map(|(tc, s, _)| (*tc, *s))
        .collect();
    subsections_in_order_output.sort_by_key(|(_, s)| *s);
    for (tc, in_start, in_end) in &in_ranges {
        let in_len = in_end - in_start;
        let (out_start, out_len) = match out_by_tc.get(tc) {
            Some((s, e)) => (Some(*s), Some(e - s)),
            None => (None, None),
        };
        let in_str = format!("offset={in_start:>6} len={in_len:>6}");
        let out_str = match (out_start, out_len) {
            (Some(s), Some(l)) => format!("offset={s:>6} len={l:>6}"),
            _ => "(missing)".to_string(),
        };
        let delta = match (out_start, out_len) {
            (Some(s), Some(l)) => {
                format!(
                    "Δoffset={:+} Δlen={:+}",
                    s as isize - *in_start as isize,
                    l as isize - in_len as isize
                )
            }
            _ => String::new(),
        };
        eprintln!(
            "  {:24}  in: {}  out: {}  {}",
            section_label(*tc),
            in_str,
            out_str,
            delta
        );
        subsections_in_order_input.push(section_label(*tc));
    }

    eprintln!("\n--- input data-section subsection order ---");
    eprintln!("  {}", subsections_in_order_input.join(" → "));
    eprintln!("\n--- output data-section subsection order ---");
    eprintln!(
        "  {}",
        subsections_in_order_output
            .iter()
            .map(|(tc, _)| section_label(*tc))
            .collect::<Vec<_>>()
            .join(" → ")
    );

    // Find first byte that differs.
    let mut first_diff: Option<usize> = None;
    for (i, (a, b)) in input_bytes.iter().zip(output_bytes.iter()).enumerate() {
        if a != b {
            first_diff = Some(i);
            break;
        }
    }
    let first_diff = match first_diff {
        Some(p) => p,
        None => {
            if input_bytes.len() != output_bytes.len() {
                eprintln!(
                    "\nfirst diff: length mismatch at common-prefix end ({} vs {})",
                    input_bytes.len(),
                    output_bytes.len()
                );
                return;
            }
            eprintln!("\nfirst diff: NONE — bytes are identical");
            return;
        }
    };

    let section = section_containing(&in_ranges, first_diff);
    eprintln!("\n--- FIRST DIFF AT byte offset {first_diff} ({first_diff:#x}) ---");
    if let Some((tc, s, e)) = section {
        let in_section_offset = first_diff - s;
        let section_len = e - s;
        eprintln!(
            "  section: {} (type_code {:#06x})",
            section_label(tc),
            tc
        );
        eprintln!(
            "  section range: {s}..{e} (len {section_len}); offset within section: {in_section_offset}"
        );
    } else {
        eprintln!("  section: (outside any map_entry range)");
    }
    let ctx_start = first_diff.saturating_sub(8);
    let ctx_len = 32;
    eprintln!(
        "  input  bytes [{}..]: {}",
        ctx_start,
        hex_dump(&input_bytes, ctx_start, ctx_len)
    );
    eprintln!(
        "  output bytes [{}..]: {}",
        ctx_start,
        hex_dump(&output_bytes, ctx_start, ctx_len)
    );
}
