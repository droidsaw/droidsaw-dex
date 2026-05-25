//! Dump parse_errors + strings count for a given DEX. Used to triage
//! Tier 4 cascade files where the map_list size disagrees by 1.

use droidsaw_dex::parser::DexFile;

#[test]
fn parse_errors_dump() {
    let Ok(path) = std::env::var("DROIDSAW_DUMP_INPUT_DEX") else {
        eprintln!("DROIDSAW_DUMP_INPUT_DEX unset; skipping");
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let dex = DexFile::parse(&bytes, None).unwrap();
    eprintln!("=== {path} ===");
    eprintln!("strings count: {}", dex.strings.len());
    eprintln!("string_data_offs count: {}", dex.string_data_offs.len());
    eprintln!("parse_errors total: {}", dex.parse_errors.len());
    let mut by_kind: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for e in &dex.parse_errors {
        *by_kind.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
    }
    for (k, v) in &by_kind {
        eprintln!("  {k}: {v}");
    }
    for e in dex.parse_errors.iter().take(5) {
        eprintln!("  first: {:?} offset={}", e.kind, e.offset);
    }
    // What does the header say about string_ids?
    eprintln!("header.string_ids_size = {}", dex.header.string_ids_size);
    eprintln!("annotation_sets count (IR): {}", dex.annotation_sets.len());
    eprintln!("annotation_items count (IR): {}", dex.annotation_items.len());
    // What does the input map_list claim?
    for e in &dex.map_entries {
        eprintln!("  map: tc={:#06x} size={} offset={}", e.type_code, e.size, e.offset);
    }
}
