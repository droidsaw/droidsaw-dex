//! Diagnose whether dex.debug_info_raw_bytes covers the full debug_info
//! section as listed in the input map. If not, identify the gap.

use droidsaw_dex::emit_dex::map_type;
use droidsaw_dex::parser::DexFile;

#[test]
fn debug_info_completeness() {
    let Ok(path) = std::env::var("DROIDSAW_DEBUG_INFO_COMPLETENESS_INPUT") else {
        eprintln!("DROIDSAW_DEBUG_INFO_COMPLETENESS_INPUT unset; skipping");
        return;
    };
    let input = std::fs::read(&path).unwrap();
    let dex = DexFile::parse(&input, None).unwrap();

    let mut sorted = dex.map_entries.clone();
    sorted.sort_by_key(|e| e.offset);
    let Some(idx) = sorted.iter().position(|e| e.type_code == map_type::DEBUG_INFO_ITEM) else {
        eprintln!("no debug_info in map_entries");
        return;
    };
    let section_start = sorted[idx].offset as usize;
    let section_size = sorted[idx].size as usize;
    let next_start = sorted.get(idx + 1).map(|n| n.offset as usize).unwrap_or(input.len());
    let derived_size = next_start - section_start;

    eprintln!("=== debug_info completeness for {path} ===");
    eprintln!("map_entries says: offset={section_start} size={section_size}");
    eprintln!("derived from next entry: byte size = {derived_size}");

    let mut offs_sorted: Vec<u32> = dex.debug_info_raw_bytes.keys().copied().collect();
    offs_sorted.sort();
    let captured_total: usize = dex.debug_info_raw_bytes.values().map(|v| v.len()).sum();
    eprintln!(
        "dex.debug_info_raw_bytes: {} entries, total captured bytes = {captured_total}",
        offs_sorted.len()
    );

    // parse_errors related to debug_info
    let dbg_errs: Vec<u32> = dex.parse_errors.iter()
        .filter(|e| matches!(e.kind, droidsaw_dex::parser::ParseFailureKind::DebugInfo))
        .map(|e| e.offset)
        .collect();
    eprintln!("parse_errors (debug_info): {} entries", dbg_errs.len());
    if !dbg_errs.is_empty() {
        eprintln!("  first 5 offsets: {:?}", &dbg_errs[..dbg_errs.len().min(5)]);
    }

    // gap analysis: walk the captured entries by ascending offset and
    // report uncovered byte ranges within the section
    let mut cur = section_start as u32;
    let mut gaps: Vec<(u32, u32)> = Vec::new();
    for off in &offs_sorted {
        let len = dex.debug_info_raw_bytes[off].len() as u32;
        if *off > cur {
            gaps.push((cur, *off - cur));
        }
        cur = off + len;
    }
    let section_end = (section_start + derived_size) as u32;
    if cur < section_end {
        gaps.push((cur, section_end - cur));
    }
    eprintln!("uncovered byte ranges within section: {} gaps", gaps.len());
    for (i, (start, len)) in gaps.iter().enumerate().take(5) {
        eprintln!("  gap #{i}: start={start} len={len}");
    }
    let total_gap_bytes: u32 = gaps.iter().map(|(_, l)| l).sum();
    eprintln!("total gap bytes: {total_gap_bytes}");
}
