//! Per-section diff summary: input vs all3-preserve output, for every
//! section in the map_list. Prints the byte diff count per section.
//!
//! Used to localize Tier 1 content_equiv_FAIL files (where the input
//! re-parses to a different IR) — by which section's bytes drift, we
//! can guess which emit-side decision is wrong.
//!
//! ```bash
//! DROIDSAW_DIFF_INPUT_DEX=/tmp/foo.dex \
//!     cargo test --release --test section_diff_summary -- --nocapture
//! ```

use droidsaw_dex::emit_dex::{emit_dex_collect, map_type, EmitConfig};
use droidsaw_dex::parser::DexFile;

fn type_code_label(tc: u16) -> &'static str {
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

#[test]
fn section_diff_summary() {
    let Ok(path) = std::env::var("DROIDSAW_DIFF_INPUT_DEX") else {
        eprintln!("DROIDSAW_DIFF_INPUT_DEX unset; skipping");
        return;
    };
    let input = std::fs::read(&path).unwrap();
    let dex = DexFile::parse(&input, None).unwrap();
    let cfg = EmitConfig {
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        preserve_data_section_layout: true,
        ..Default::default()
    };
    let out = match emit_dex_collect(&dex, &cfg) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("EMIT FAILED: {e:?}");
            return;
        }
    };

    let mut sorted = dex.map_entries.clone();
    sorted.sort_by_key(|e| e.offset);
    eprintln!("\n=== section diff summary for {path} ===");
    eprintln!("input_len={} output_len={}", input.len(), out.bytes.len());

    let len = input.len().min(out.bytes.len());
    for (i, e) in sorted.iter().enumerate() {
        let start = (e.offset as usize).min(len);
        let end = sorted
            .get(i + 1)
            .map(|n| (n.offset as usize).min(len))
            .unwrap_or(len);
        if start >= end {
            continue;
        }
        let diff: usize = (start..end)
            .filter(|&p| input.get(p) != out.bytes.get(p))
            .count();
        let pct = 100.0 * (diff as f64) / ((end - start) as f64);
        if diff > 0 {
            eprintln!(
                "  {:<24}  start={start:>10}  len={:>8}  diff={diff:>8} ({pct:.2}%)",
                type_code_label(e.type_code),
                end - start
            );
        }
    }
    let total_diff: usize = (0..len)
        .filter(|&p| input.get(p) != out.bytes.get(p))
        .count();
    eprintln!("  TOTAL position-wise diff: {total_diff}");
}
