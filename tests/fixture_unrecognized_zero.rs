//! Regression sentinel — every in-tree DEX fixture must produce zero
//! UNRECOGNIZED_REGION findings. If a future change introduces an
//! unrecognized region on a stable fixture, this test fails (catching
//! the regression before the per-APK ratchet at
//! `tests/unrecognized_ratchet.rs` even runs).
//!
//! Discipline: "Existing fixture corpus — zero unrecognized (regression
//! sentinel)."
//!
//! Aggregate-zero rather than per-fixture-tracked: fixtures are
//! designed to compile_pass / compile_fail / semantic_fail cleanly; an
//! unrecognized region in a fixture is a regression, not a graduate.
//! No need to bump `droidsaw-fixture-harness::MANIFEST_VERSION` for a
//! per-fixture `unrecognized_count: u32` column representing a
//! constant 0.

use std::fs;
use std::path::{Path, PathBuf};

fn fixture_dex(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn assert_zero_unrecognized(path: &Path) {
    let data = fs::read(path).expect("read fixture dex");
    let dex = droidsaw_dex::parser::DexFile::parse(&data, None).expect("parse fixture dex");
    let findings = droidsaw_dex::diag::collect_unrecognized_findings(&dex, &data);
    if !findings.is_empty() {
        let details: Vec<&str> = findings.iter().map(|f| f.detail.as_str()).collect();
        panic!(
            "{}: expected 0 UNRECOGNIZED_REGION findings on stable fixture; got {}:\n  - {}",
            path.display(),
            findings.len(),
            details.join("\n  - "),
        );
    }
}

#[test]
fn classes_dex_has_no_unrecognized_regions() {
    assert_zero_unrecognized(&fixture_dex("classes.dex"));
}

#[test]
fn classes_named_dex_has_no_unrecognized_regions() {
    assert_zero_unrecognized(&fixture_dex("classes_named.dex"));
}
