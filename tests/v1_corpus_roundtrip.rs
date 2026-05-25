//! v1-corpus-dex-claims claim 2 harness: tolerant roundtrip measurement.
//!
//! Mirrors corpus_emit_smoke but with continue-on-error semantics so we
//! can produce per-claim statistics across the full corpus rather than
//! halting at the first divergence. Categorizes each DEX as:
//!   - parse_fail: DexFile::parse failed outright
//!   - emit_partial_ir: parse succeeded but had soft errors; emit refused
//!   - emit_not_implemented
//!   - emit_other_err
//!   - emit_ok + byte_identical
//!   - emit_ok + byte_diff (content-equivalent but transforms applied)
//!
//! Env-gated. Run with:
//!   DROIDSAW_DEX_CORPUS=/tmp/v1cdc-dex-corpus \
//!     cargo test --release --test v1_corpus_roundtrip -- --nocapture

use std::path::PathBuf;

use droidsaw_dex::emit_dex::{emit_dex_collect, DexEmitError, EmitConfig};
use droidsaw_dex::parser::{ContentEquiv, DexFile};

fn corpus_root() -> Option<PathBuf> {
    std::env::var("DROIDSAW_DEX_CORPUS").ok().map(PathBuf::from)
}

fn walk_dex(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { stack.push(p); }
            else if p.extension().and_then(|s| s.to_str()) == Some("dex") { out.push(p); }
        }
    }
    out.sort();
    out
}

#[test]
fn v1_corpus_roundtrip_stats() {
    let Some(root) = corpus_root() else {
        eprintln!("v1_corpus_roundtrip_stats: DROIDSAW_DEX_CORPUS unset; skipping");
        return;
    };
    let dex_paths = walk_dex(&root);
    if dex_paths.is_empty() {
        eprintln!("SKIP: corpus root {root:?} has no .dex files");
        return;
    }

    let total = dex_paths.len();
    let mut parse_fail = 0usize;
    let mut emit_partial_ir = 0usize;
    let mut emit_not_implemented = 0usize;
    let mut emit_other_err = 0usize;
    let mut emit_ok = 0usize;
    let mut byte_identical = 0usize;
    let mut content_equiv_only = 0usize;
    let mut content_equiv_fail = 0usize;
    let mut reparse_fail = 0usize;

    for (i, path) in dex_paths.iter().enumerate() {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => { parse_fail += 1; continue; }
        };
        let dex1 = match DexFile::parse(&bytes, None) {
            Ok(d) => d,
            Err(_) => { parse_fail += 1; continue; }
        };
        let out = match emit_dex_collect(&dex1, &EmitConfig::default()) {
            Ok(o) => o,
            Err(DexEmitError::NotImplemented) => { emit_not_implemented += 1; continue; }
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("parse error") || msg.contains("permit_partial_ir") {
                    emit_partial_ir += 1;
                } else {
                    emit_other_err += 1;
                    eprintln!("  emit_other: {} → {msg}", path.display());
                }
                continue;
            }
        };
        emit_ok += 1;
        if bytes == out.bytes {
            byte_identical += 1;
            continue;
        }
        match DexFile::parse(&out.bytes, None) {
            Ok(dex2) => {
                if ContentEquiv(&dex1) == ContentEquiv(&dex2) {
                    content_equiv_only += 1;
                } else {
                    content_equiv_fail += 1;
                    eprintln!("  content-equiv FAIL: {}", path.display());
                }
            }
            Err(_) => {
                reparse_fail += 1;
                eprintln!("  reparse FAIL: {}", path.display());
            }
        }

        if (i + 1) % 100 == 0 {
            eprintln!(
                "  [{}/{total}] checkpoint: parse_fail={parse_fail}, partial_ir={emit_partial_ir}, ni={emit_not_implemented}, emit_ok={emit_ok}, byte_id={byte_identical}",
                i + 1
            );
        }
    }

    eprintln!();
    eprintln!("## v1_corpus_roundtrip_stats");
    eprintln!();
    eprintln!("Corpus root: {}", root.display());
    eprintln!("Total DEX walked: {total}");
    eprintln!();
    eprintln!("| Bucket | Count | % |");
    eprintln!("|---|---|---|");
    let pct = |n: usize| if total == 0 { 0.0 } else { 100.0 * (n as f64) / (total as f64) };
    eprintln!("| parse_fail | {parse_fail} | {:.2}% |", pct(parse_fail));
    eprintln!("| emit_partial_ir (parser flagged soft errors) | {emit_partial_ir} | {:.2}% |", pct(emit_partial_ir));
    eprintln!("| emit_not_implemented | {emit_not_implemented} | {:.2}% |", pct(emit_not_implemented));
    eprintln!("| emit_other_err | {emit_other_err} | {:.2}% |", pct(emit_other_err));
    eprintln!("| emit_ok | {emit_ok} | {:.2}% |", pct(emit_ok));
    eprintln!("|  ↳ byte_identical | {byte_identical} | {:.2}% |", pct(byte_identical));
    eprintln!("|  ↳ content_equiv_only | {content_equiv_only} | {:.2}% |", pct(content_equiv_only));
    eprintln!("|  ↳ content_equiv_fail | {content_equiv_fail} | {:.2}% |", pct(content_equiv_fail));
    eprintln!("|  ↳ reparse_fail | {reparse_fail} | {:.2}% |", pct(reparse_fail));
    let denom = emit_ok;
    if denom > 0 {
        eprintln!();
        eprintln!("Among emit_ok ({denom}):");
        eprintln!("  byte-identity rate: {:.2}%", 100.0 * (byte_identical as f64) / (denom as f64));
        eprintln!("  content-equiv rate (incl. byte-identical): {:.2}%", 100.0 * ((byte_identical + content_equiv_only) as f64) / (denom as f64));
    }
}
