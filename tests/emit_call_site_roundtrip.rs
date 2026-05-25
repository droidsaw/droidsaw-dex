//! call-site-id round-trip gate: round-trip a DEX that retains
//! `call_site_ids` + `method_handles` sections (d8 `--no-desugaring`
//! on invoke-custom/polymorphic inputs). The emit path writes both
//! sections + per-call-site encoded_array data + map-list entries.
//!
//! No committed fixture (DEX bytes are gitignored + require
//! `javac + d8 --min-api 26 --no-desugaring` to produce, which isn't
//! the regen.sh default). Test is env-gated on toolchain
//! availability — skips cleanly in CI without javac/d8; runs
//! locally via `ANDROID_HOME=... cargo test`.

use std::path::PathBuf;
use std::process::Command;

use droidsaw_dex::emit_dex::emit_dex;
use droidsaw_dex::parser::{ContentEquiv, DexFile};
use droidsaw_fixture_harness::{check_warnings_strict, skipped_outcome};

fn resolve_javac() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(&home).join("bin").join("javac");
        if p.exists() {
            return Some(p);
        }
    }
    let out = Command::new("which").arg("javac").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

fn resolve_d8() -> Option<PathBuf> {
    if let Ok(android) = std::env::var("ANDROID_HOME") {
        let bt = PathBuf::from(&android).join("build-tools");
        if let Ok(entries) = std::fs::read_dir(&bt) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            versions.sort();
            versions.reverse();
            for v in versions {
                let d8 = v.join("d8");
                if d8.exists() {
                    return Some(d8);
                }
            }
        }
    }
    None
}

/// Build a DEX with `call_site_ids` + `method_handles` populated,
/// then parse-emit-parse and assert ContentEquiv. Skips cleanly when
/// javac/d8 are unavailable (matches the `fixture_ratchet` pattern).
#[test]
fn call_site_roundtrip_via_lambdas_fixture() {
    let (javac, d8) = match (resolve_javac(), resolve_d8()) {
        (Some(a), Some(b)) => (a, b),
        (a, b) => {
            let mut skip_outcomes = Vec::new();
            if a.is_none() {
                skip_outcomes.push(skipped_outcome(
                    "call_site_roundtrip_via_lambdas_fixture",
                    "javac",
                    "javac not found",
                ));
            }
            if b.is_none() {
                skip_outcomes.push(skipped_outcome(
                    "call_site_roundtrip_via_lambdas_fixture",
                    "d8",
                    "d8 not found",
                ));
            }
            for o in &skip_outcomes {
                for w in &o.warnings {
                    eprintln!("SKIP: {w:?}");
                }
            }
            check_warnings_strict(&skip_outcomes).expect("strict-warnings gate");
            return;
        }
    };

    // Use the committed Lambdas source; rebuild here with
    // `--no-desugaring` to retain invoke-custom on the output (the
    // default `fixture_ratchet` d8 run desugars these to synthetic
    // classes).
    let src_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/java/Lambdas/src.java");
    if !src_path.exists() {
        eprintln!("call_site_roundtrip: Lambdas src missing; skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tmpdir");
    let classes_dir = tmp.path().join("classes");
    std::fs::create_dir_all(&classes_dir).expect("mkdir classes");

    // javac rejects public class X from a file not literally named
    // X.java — copy src.java to the tmp with the declared class
    // name as the filename (manifest's fixture dir layout uses
    // `<Name>/src.java` instead).
    let staged_src = tmp.path().join("Lambdas.java");
    std::fs::copy(&src_path, &staged_src).expect("copy src");

    let javac_status = Command::new(&javac)
        .args(["-d"])
        .arg(&classes_dir)
        .arg(&staged_src)
        .status()
        .expect("javac spawn");
    assert!(javac_status.success(), "javac failed");

    let dex_dir = tmp.path().join("dex");
    std::fs::create_dir_all(&dex_dir).expect("mkdir dex");
    let class_files: Vec<PathBuf> = std::fs::read_dir(&classes_dir)
        .expect("read classes_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("class"))
        .collect();
    assert!(!class_files.is_empty(), "no class files produced");

    let mut d8_cmd = Command::new(&d8);
    d8_cmd.args(["--min-api", "26", "--no-desugaring", "--output"]);
    d8_cmd.arg(&dex_dir);
    for cf in &class_files {
        d8_cmd.arg(cf);
    }
    let d8_status = d8_cmd.status().expect("d8 spawn");
    assert!(d8_status.success(), "d8 failed");

    let dex_path = dex_dir.join("classes.dex");
    let bytes = std::fs::read(&dex_path).expect("read dex");
    let dex = DexFile::parse(&bytes, None).expect("parse");
    assert!(
        !dex.call_site_ids.is_empty(),
        "Lambdas with --no-desugaring should populate call_site_ids"
    );
    assert!(
        !dex.method_handles.is_empty(),
        "Lambdas with --no-desugaring should populate method_handles"
    );

    // Round-trip: parse → emit → parse → ContentEquiv.
    let emitted = emit_dex(&dex).expect("emit succeeds on call_site IR");
    let dex2 = DexFile::parse(&emitted, None).expect("re-parse succeeds");
    assert_eq!(dex.call_site_ids.len(), dex2.call_site_ids.len());
    assert_eq!(dex.method_handles.len(), dex2.method_handles.len());
    assert_eq!(
        ContentEquiv(&dex),
        ContentEquiv(&dex2),
        "content-equivalence broke on call_site round-trip"
    );
}
