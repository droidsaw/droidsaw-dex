//! PoC: demonstrate that droidsaw's parse-emit-parse round-trip changes
//! the DEX bytes for ~all real-world inputs, which would break any APK
//! signature scheme (v1/v2/v3/v4) that hashes over the DEX entry bytes.
//!
//! This test is opt-in: gated on `DROIDSAW_SIGBREAK_POC_APK=<path-to-apk>`.
//! Reads `classes.dex` out of the APK (via stdlib `zip` is unavailable here
//! so the test takes a raw DEX path via `DROIDSAW_SIGBREAK_POC_DEX` instead;
//! a small shell wrapper unzips the APK first).
//!
//! Run:
//!
//! ```bash
//! DROIDSAW_SIGBREAK_POC_DEX=/tmp/poc-input.dex \
//!     cargo test --release --test signature_break_poc -- --nocapture
//! ```
//!
//! Output: original SHA-256, roundtripped SHA-256, written roundtripped
//! bytes to `/tmp/poc-roundtripped.dex`. The test PASSES iff the SHAs
//! differ — i.e., the PoC succeeds in demonstrating the byte-change.
//! (Conventional "test PASSES iff correctness holds" is inverted here
//! because the demonstrated fact IS the byte non-identity.)

use sha2::{Digest, Sha256};

use droidsaw_dex::emit_dex::{emit_dex_collect, EmitConfig};
use droidsaw_dex::parser::DexFile;

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

#[test]
fn signature_break_poc_dex_byte_change() {
    let Ok(path) = std::env::var("DROIDSAW_SIGBREAK_POC_DEX") else {
        eprintln!(
            "signature_break_poc: DROIDSAW_SIGBREAK_POC_DEX unset; \
             skipping (set to a path to a real DEX file extracted from \
             a signed APK to run)"
        );
        return;
    };
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let original_sha = sha256_hex(&bytes);
    eprintln!("\nsignature_break_poc: input dex = {path}");
    eprintln!("  input_bytes_len  = {}", bytes.len());
    eprintln!("  input_sha256     = {original_sha}");

    let dex = DexFile::parse(&bytes, None).expect("parse input dex");

    // Use the MAXIMUM preservation we can: all three toggles on
    // (preserve_data_section_layout is the byte-identity unlock).
    let cfg = EmitConfig {
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        preserve_data_section_layout: true,
        ..Default::default()
    };
    let out = emit_dex_collect(&dex, &cfg).expect("emit roundtripped dex");
    let rt_sha = sha256_hex(&out.bytes);
    eprintln!("  output_bytes_len = {}", out.bytes.len());
    eprintln!("  output_sha256    = {rt_sha}");

    let out_path = "/tmp/poc-roundtripped.dex";
    std::fs::write(out_path, &out.bytes).expect("write output");
    eprintln!("  wrote roundtripped DEX to: {out_path}");

    let diff_count = bytes
        .iter()
        .zip(out.bytes.iter())
        .filter(|(a, b)| a != b)
        .count()
        + bytes.len().abs_diff(out.bytes.len());
    eprintln!("  diff_bytes (position-wise) = {diff_count}");
    eprintln!(
        "  applied transforms: {:?}",
        out.applied_transformations
    );

    if original_sha == rt_sha {
        eprintln!(
            "\nROUNDTRIP DEMONSTRATED: droidsaw parse-emit-parse with all 3 \
             preserve toggles produces a BYTE-IDENTICAL DEX. APK re-pack + \
             signature verification (apksigner v3) will PASS."
        );
    } else {
        eprintln!(
            "\nROUNDTRIP IMPERFECT: SHAs differ even with all 3 preserve toggles. \
             This DEX is in the set of inputs the v1 preserve_data_section_layout \
             toggle doesn't yet fully cover (see corpus_content_equiv_triage)."
        );
    }
    // Inverted assertion (was assert_ne pre-v1.x): under all-preserve mode, the
    // expected outcome on canonical d8 input is SHA-match. The test PASSES on
    // byte-identity. Files in the residual-divergence tail (rare) will fail
    // here and surface the file path for follow-up triage.
    assert_eq!(
        original_sha, rt_sha,
        "preserve mode failed to produce byte-identical roundtrip for this DEX \
         — file is in the residual tail of inputs that need further fix"
    );
}
