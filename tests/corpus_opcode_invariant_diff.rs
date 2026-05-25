//! Env-gated corpus differential for the four opcode-invariant findings.
//! Walks `*.apk` + `*.dex` under `$DROIDSAW_DEX_CORPUS`, extracts top-level
//! `classes*.dex` from each APK, parses each DEX, and tallies the four
//! `FINDING_ID_DEX_*` constants that map to matrix cells H-1 / M-1 /
//! M-2 / UNCHECKED-1.
//!
//! Mirrors `tests/corpus_emit_smoke.rs` — env unset → no-op (CI stays
//! green). Tally-only: per-input non-zero rates and grand totals are
//! printed to stderr; the test never panics on a non-zero finding count
//! (we are *measuring* baseline rates, not gating on them).
//!
//! ```bash
//! DROIDSAW_DEX_CORPUS=~/code/re/ \
//!     cargo test --release --test corpus_opcode_invariant_diff -- --nocapture
//! ```
//!
//! Per-input ZIP extraction filter mirrors `droidsaw-apk`'s shape —
//! top-level `classes*.dex` only (no subdirectory matches), DEX magic
//! `b"dex\n"` required. Per-entry read is capped at 512 MiB to mirror
//! `droidsaw-apk`'s `MAX_ENTRY_BYTES`.

use std::io::Read;
use std::path::{Path, PathBuf};

use droidsaw_common::finding::Finding;
use droidsaw_dex::diag::{
    collect_code_item_findings, FINDING_ID_DEX_BRANCH_TARGET_OUT_OF_RANGE,
    FINDING_ID_DEX_OPCODE_ARG_COUNT_OUT_OF_RANGE, FINDING_ID_DEX_PAYLOAD_IDENT_MISMATCH,
    FINDING_ID_DEX_UNKNOWN_OPCODE_BYTE,
};
use droidsaw_dex::parser::DexFile;

const DEX_MAGIC: [u8; 4] = *b"dex\n";
const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Default, Debug, Clone)]
struct Tallies {
    /// `FINDING_ID_DEX_OPCODE_ARG_COUNT_OUT_OF_RANGE` (matrix H-1).
    h1: usize,
    /// `FINDING_ID_DEX_BRANCH_TARGET_OUT_OF_RANGE` (matrix M-1).
    m1: usize,
    /// `FINDING_ID_DEX_PAYLOAD_IDENT_MISMATCH` (matrix M-2).
    m2: usize,
    /// `FINDING_ID_DEX_UNKNOWN_OPCODE_BYTE` (matrix UNCHECKED-1).
    unchecked1: usize,
}

impl Tallies {
    fn from_findings(findings: &[Finding]) -> Self {
        let mut t = Self::default();
        for f in findings {
            match f.id.as_str() {
                FINDING_ID_DEX_OPCODE_ARG_COUNT_OUT_OF_RANGE => t.h1 += 1,
                FINDING_ID_DEX_BRANCH_TARGET_OUT_OF_RANGE => t.m1 += 1,
                FINDING_ID_DEX_PAYLOAD_IDENT_MISMATCH => t.m2 += 1,
                FINDING_ID_DEX_UNKNOWN_OPCODE_BYTE => t.unchecked1 += 1,
                _ => {}
            }
        }
        t
    }

    fn add(&mut self, other: &Self) {
        self.h1 += other.h1;
        self.m1 += other.m1;
        self.m2 += other.m2;
        self.unchecked1 += other.unchecked1;
    }

    fn is_zero(&self) -> bool {
        self.h1 == 0 && self.m1 == 0 && self.m2 == 0 && self.unchecked1 == 0
    }
}

fn corpus_root() -> Option<PathBuf> {
    std::env::var("DROIDSAW_DEX_CORPUS").ok().map(PathBuf::from)
}

/// Top-level `classes*.dex` filter mirroring `droidsaw-apk`'s shape.
/// Rejects entries with path separators (subdirectory matches) and any
/// name not matching `^classes[0-9]*\.dex$`.
fn is_classes_dex(name: &str) -> bool {
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    let Some(stem) = name.strip_suffix(".dex") else {
        return false;
    };
    let Some(rest) = stem.strip_prefix("classes") else {
        return false;
    };
    rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit())
}

fn walk_inputs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "apk" || ext == "dex" {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out
}

/// Returns each top-level `classes*.dex` entry as `(name, bytes)` if the
/// archive opens and the entry passes the DEX-magic check. Returns empty
/// on archive open / read failure (caller treats as a per-input skip).
fn extract_classes_dex(apk: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let Ok(file) = std::fs::File::open(apk) else {
        return out;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return out;
    };
    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else {
            continue;
        };
        let name = entry.name().to_string();
        if !is_classes_dex(&name) {
            continue;
        }
        let mut buf = Vec::new();
        if entry
            .by_ref()
            .take(MAX_ENTRY_BYTES)
            .read_to_end(&mut buf)
            .is_err()
        {
            continue;
        }
        if buf.len() < 4 || buf[0..4] != DEX_MAGIC {
            continue;
        }
        out.push((name, buf));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn corpus_opcode_invariant_diff() {
    let Some(root) = corpus_root() else {
        eprintln!(
            "corpus_opcode_invariant_diff: DROIDSAW_DEX_CORPUS unset; skipping \
             (set to a directory of .apk / .dex files to run)"
        );
        return;
    };
    let inputs = walk_inputs(&root);
    if inputs.is_empty() {
        eprintln!("SKIP: corpus root {root:?} has no .apk / .dex files");
        return;
    }

    let total_inputs = inputs.len();
    let mut per_input: Vec<(String, Tallies, usize)> = Vec::new();
    let mut grand = Tallies::default();
    let mut unreadable_inputs = 0usize;
    let mut parse_failed_dex = 0usize;
    let mut total_dex_parsed = 0usize;

    for (i, input) in inputs.iter().enumerate() {
        let display = input
            .strip_prefix(&root)
            .unwrap_or(input)
            .display()
            .to_string();

        let dexes: Vec<(String, Vec<u8>)> =
            if input.extension().and_then(|s| s.to_str()) == Some("dex") {
                match std::fs::read(input) {
                    Ok(bytes) => {
                        let name = input
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        vec![(name, bytes)]
                    }
                    Err(_) => {
                        unreadable_inputs += 1;
                        continue;
                    }
                }
            } else {
                extract_classes_dex(input)
            };

        if dexes.is_empty() {
            unreadable_inputs += 1;
            continue;
        }

        let mut input_tallies = Tallies::default();
        let mut input_dex_count = 0usize;
        for (_dex_name, bytes) in &dexes {
            let dex = match DexFile::parse(bytes, None) {
                Ok(d) => d,
                Err(_) => {
                    parse_failed_dex += 1;
                    continue;
                }
            };
            let findings = collect_code_item_findings(&dex);
            input_tallies.add(&Tallies::from_findings(&findings));
            input_dex_count += 1;
            total_dex_parsed += 1;
        }
        grand.add(&input_tallies);
        per_input.push((display, input_tallies, input_dex_count));

        if (i + 1) % 25 == 0 {
            eprintln!(
                "  [{}/{total_inputs}] checkpoint: {total_dex_parsed} DEX parsed, \
                 grand=(h1={}, m1={}, m2={}, unchecked1={})",
                i + 1,
                grand.h1,
                grand.m1,
                grand.m2,
                grand.unchecked1,
            );
        }
    }

    let denom = total_inputs - unreadable_inputs;
    let pct = |n: usize| -> f64 {
        if denom == 0 {
            0.0
        } else {
            100.0 * (n as f64) / (denom as f64)
        }
    };

    eprintln!();
    eprintln!("## opcode-invariant corpus baseline");
    eprintln!();
    eprintln!("Corpus root: `{}`", root.display());
    eprintln!("Inputs walked: {total_inputs} (`*.apk` + `*.dex`)");
    eprintln!("Inputs with extractable DEX: {denom}");
    eprintln!("Total DEX payloads parsed: {total_dex_parsed} ({parse_failed_dex} parse-fail)");
    eprintln!("Inputs unreadable / no `classes*.dex`: {unreadable_inputs}");
    eprintln!();
    eprintln!("### Per-finding totals");
    eprintln!();
    eprintln!("| Finding ID | Matrix cell | Total fires | Inputs ≥1 fire | Per-input rate |");
    eprintln!("|---|---|---|---|---|");
    let id_rows = [
        (
            "DEX_OPCODE_ARG_COUNT_OUT_OF_RANGE",
            "H-1",
            grand.h1,
            per_input.iter().filter(|(_, t, _)| t.h1 > 0).count(),
        ),
        (
            "DEX_BRANCH_TARGET_OUT_OF_RANGE",
            "M-1",
            grand.m1,
            per_input.iter().filter(|(_, t, _)| t.m1 > 0).count(),
        ),
        (
            "DEX_PAYLOAD_IDENT_MISMATCH",
            "M-2",
            grand.m2,
            per_input.iter().filter(|(_, t, _)| t.m2 > 0).count(),
        ),
        (
            "DEX_UNKNOWN_OPCODE_BYTE",
            "UNCHECKED-1",
            grand.unchecked1,
            per_input.iter().filter(|(_, t, _)| t.unchecked1 > 0).count(),
        ),
    ];
    for (name, cell, total, hits) in &id_rows {
        eprintln!(
            "| `{name}` | {cell} | {total} | {hits} | {:.2}% |",
            pct(*hits)
        );
    }
    eprintln!();
    eprintln!("### Per-input tallies (non-zero only)");
    eprintln!();
    eprintln!("| Input | DEX# | H-1 | M-1 | M-2 | UNCHECKED-1 |");
    eprintln!("|---|---|---|---|---|---|");
    let mut non_zero = 0usize;
    for (name, t, n) in &per_input {
        if t.is_zero() {
            continue;
        }
        non_zero += 1;
        eprintln!(
            "| `{name}` | {n} | {} | {} | {} | {} |",
            t.h1, t.m1, t.m2, t.unchecked1
        );
    }
    eprintln!();
    eprintln!("Inputs with ≥1 finding: {non_zero}/{denom}");
}

#[test]
fn is_classes_dex_classifier() {
    assert!(is_classes_dex("classes.dex"));
    assert!(is_classes_dex("classes2.dex"));
    assert!(is_classes_dex("classes10.dex"));
    assert!(!is_classes_dex("base/dex/classes.dex"));
    assert!(!is_classes_dex("subdir/classes.dex"));
    assert!(!is_classes_dex("classes.dex.bak"));
    assert!(!is_classes_dex("classesx.dex"));
    assert!(!is_classes_dex("other.dex"));
    assert!(!is_classes_dex("classes"));
}
