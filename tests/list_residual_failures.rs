//! Walk a corpus and bucket every non-byte-identical file with its
//! root-cause class + error message. Used to plan the v1 residual fixes
//! (not just count them).
//!
//! ```bash
//! DROIDSAW_DEX_CORPUS=/tmp/v1-roundtrip-corpus \
//! DROIDSAW_LIST_RESIDUAL=1 \
//!     cargo test --release --test list_residual_failures -- --nocapture --test-threads=1
//! ```

use std::path::{Path, PathBuf};

use droidsaw_dex::emit_dex::{emit_dex_collect, EmitConfig};
use droidsaw_dex::parser::DexFile;

fn walk(root: &Path, acc: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, acc);
        } else if p.extension().is_some_and(|e| e == "dex") {
            acc.push(p);
        }
    }
}

#[test]
fn list_residual_failures() {
    if std::env::var("DROIDSAW_LIST_RESIDUAL").ok().as_deref() != Some("1") {
        eprintln!("DROIDSAW_LIST_RESIDUAL != 1; skipping");
        return;
    }
    let Some(root) = std::env::var("DROIDSAW_DEX_CORPUS").ok() else {
        eprintln!("DROIDSAW_DEX_CORPUS unset; skipping");
        return;
    };
    let root = PathBuf::from(root);
    let mut paths = Vec::new();
    walk(&root, &mut paths);
    paths.sort();
    let cfg = EmitConfig {
        preserve_data_section_layout: true,
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        ..Default::default()
    };
    let mut parse_fails: Vec<(String, String)> = Vec::new();
    let mut emit_fails: Vec<(String, String)> = Vec::new();
    let mut reparse_fails: Vec<(String, String)> = Vec::new();
    let content_fails: Vec<String> = Vec::new();
    let mut tier4: Vec<(String, usize)> = Vec::new();
    let mut byte_id = 0usize;

    for path in &paths {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let dex = match DexFile::parse(&bytes, None) {
            Ok(d) => d,
            Err(e) => {
                parse_fails.push((path.display().to_string(), format!("{e:?}")));
                continue;
            }
        };
        let out = match emit_dex_collect(&dex, &cfg) {
            Ok(o) => o,
            Err(e) => {
                emit_fails.push((path.display().to_string(), format!("{e:?}")));
                continue;
            }
        };
        if bytes == out.bytes {
            byte_id += 1;
            continue;
        }
        match DexFile::parse(&out.bytes, None) {
            Ok(_) => {}
            Err(e) => {
                reparse_fails.push((path.display().to_string(), format!("{e:?}")));
                continue;
            }
        }
        let diff_total: usize = bytes
            .iter()
            .zip(out.bytes.iter())
            .filter(|(a, b)| a != b)
            .count();
        tier4.push((path.display().to_string(), diff_total));
    }

    eprintln!("\n=== residual classification ({} files total) ===", paths.len());
    eprintln!("  byte_identical : {byte_id}");
    eprintln!("  parse_fails    : {}", parse_fails.len());
    eprintln!("  emit_fails     : {}", emit_fails.len());
    eprintln!("  reparse_fails  : {}", reparse_fails.len());
    eprintln!("  content_fails  : {}", content_fails.len());
    eprintln!("  tier4_cascade  : {}", tier4.len());

    eprintln!("\n--- parse_fails ---");
    for (p, e) in &parse_fails {
        eprintln!("  {e}\n    {p}");
    }
    eprintln!("\n--- emit_fails ---");
    for (p, e) in &emit_fails {
        eprintln!("  {e}\n    {p}");
    }
    eprintln!("\n--- tier4 (diff_bytes) ---");
    for (p, d) in &tier4 {
        eprintln!("  diff={d}  {p}");
    }
}
