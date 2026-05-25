//! Diagnostic: parse a DEX, emit with all3 preserve, attempt to parse
//! the output, and report the exact parse error (path + message). Used
//! to triage Tier 1 reparse_err files from corpus_tier_ladder.
//!
//! ```bash
//! DROIDSAW_REPARSE_DIAG_INPUT_DEX=/tmp/foo.dex \
//!     cargo test --release --test reparse_diagnostic -- --nocapture
//! ```

use droidsaw_dex::emit_dex::{emit_dex_collect, EmitConfig};
use droidsaw_dex::parser::DexFile;

#[test]
fn reparse_err_locator() {
    let Ok(path) = std::env::var("DROIDSAW_REPARSE_DIAG_INPUT_DEX") else {
        eprintln!("reparse_err_locator: DROIDSAW_REPARSE_DIAG_INPUT_DEX unset; skipping");
        return;
    };
    let input_bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let dex_in = match DexFile::parse(&input_bytes, None) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("INPUT PARSE FAILED: {e:?}");
            return;
        }
    };
    let cfg = EmitConfig {
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        preserve_data_section_layout: true,
        ..Default::default()
    };
    let out = match emit_dex_collect(&dex_in, &cfg) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("EMIT FAILED: {e:?}");
            return;
        }
    };
    eprintln!("\n=== reparse_err_locator: {path} ===");
    eprintln!("input_len={} output_len={}", input_bytes.len(), out.bytes.len());
    eprintln!("applied_transforms: {:?}", out.applied_transformations);
    match DexFile::parse(&out.bytes, None) {
        Ok(_) => eprintln!("REPARSE OK — file is NOT in reparse_err set"),
        Err(e) => {
            eprintln!("\n=== REPARSE ERROR ===");
            eprintln!("{e:?}");
            eprintln!();
            let dump_path = format!("/tmp/reparse-fail-{}.dex",
                std::path::Path::new(&path).file_name().unwrap().to_string_lossy());
            std::fs::write(&dump_path, &out.bytes).ok();
            eprintln!("output bytes written to: {dump_path}");
        }
    }
}
