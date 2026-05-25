//! Find the first byte that differs between input and parse-emit output,
//! restricted to a specific named section (skips Adler/SHA cascade).
//!
//! Usage:
//! ```bash
//! DROIDSAW_DIFF_INPUT_DEX=/tmp/foo.dex \
//! DROIDSAW_DIFF_SECTION=debug_info \
//!     cargo test --release --test first_diff_in_section -- --nocapture
//! ```
//!
//! Recognized section names: any label from
//! `corpus_emit_smoke::type_code_label`.

use droidsaw_dex::emit_dex::{emit_dex_collect, map_type, EmitConfig};
use droidsaw_dex::parser::DexFile;

fn label_to_code(s: &str) -> Option<u16> {
    Some(match s {
        "header" => map_type::HEADER_ITEM,
        "string_ids" => map_type::STRING_ID_ITEM,
        "type_ids" => map_type::TYPE_ID_ITEM,
        "proto_ids" => map_type::PROTO_ID_ITEM,
        "field_ids" => map_type::FIELD_ID_ITEM,
        "method_ids" => map_type::METHOD_ID_ITEM,
        "class_defs" => map_type::CLASS_DEF_ITEM,
        "call_site_ids" => map_type::CALL_SITE_ID_ITEM,
        "method_handles" => map_type::METHOD_HANDLE_ITEM,
        "string_data" => map_type::STRING_DATA_ITEM,
        "type_list" => map_type::TYPE_LIST,
        "code_item" => map_type::CODE_ITEM,
        "class_data" => map_type::CLASS_DATA_ITEM,
        "annotation" => map_type::ANNOTATION_ITEM,
        "annotation_set" => map_type::ANNOTATION_SET_ITEM,
        "annotation_set_ref_list" => map_type::ANNOTATION_SET_REF_LIST,
        "annotation_directory" => map_type::ANNOTATION_DIRECTORY_ITEM,
        "encoded_array" => map_type::ENCODED_ARRAY_ITEM,
        "debug_info" => map_type::DEBUG_INFO_ITEM,
        "map_list" => map_type::MAP_LIST,
        _ => return None,
    })
}

#[test]
fn first_diff_in_section() {
    let Ok(path) = std::env::var("DROIDSAW_DIFF_INPUT_DEX") else {
        eprintln!("DROIDSAW_DIFF_INPUT_DEX unset; skipping");
        return;
    };
    let Ok(section) = std::env::var("DROIDSAW_DIFF_SECTION") else {
        eprintln!("DROIDSAW_DIFF_SECTION unset; skipping");
        return;
    };
    let Some(tc) = label_to_code(&section) else {
        eprintln!("unknown section: {section}");
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
    let Some(idx) = sorted.iter().position(|e| e.type_code == tc) else {
        eprintln!("section not in map_entries: {section}");
        return;
    };
    let start = sorted[idx].offset as usize;
    let end = sorted.get(idx + 1).map(|n| n.offset as usize).unwrap_or(input.len());
    let end = end.min(out.bytes.len());

    eprintln!("section={section} range={}..{} (len={})", start, end, end - start);

    let mut first_diff: Option<usize> = None;
    for i in start..end {
        if input.get(i) != out.bytes.get(i) {
            first_diff = Some(i);
            break;
        }
    }
    match first_diff {
        None => eprintln!("no diff in {section}"),
        Some(pos) => {
            let off_in_section = pos - start;
            let ctx_start = pos.saturating_sub(16);
            let ctx_end = (pos + 32).min(input.len()).min(out.bytes.len());
            eprintln!(
                "FIRST DIFF in {section} at file pos={pos} (offset within section={off_in_section})"
            );
            eprintln!(
                "  input  [{ctx_start}..]: {}",
                input[ctx_start..ctx_end].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
            );
            eprintln!(
                "  output [{ctx_start}..]: {}",
                out.bytes[ctx_start..ctx_end].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
            );
            // Count total diff bytes in this section
            let total: usize = (start..end)
                .filter(|&i| input.get(i) != out.bytes.get(i))
                .count();
            eprintln!("total diff bytes in {section}: {total}");
        }
    }
}
