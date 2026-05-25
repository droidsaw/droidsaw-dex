//! Probe parse_class_data at a specific offset and capture the exact
//! error. Used to triage Tier B (PartialIR ClassData) residual files.
//!
//! ```bash
//! DROIDSAW_PROBE_CD_INPUT=/tmp/foo.dex \
//! DROIDSAW_PROBE_CD_OFFSETS=8702548,8785813 \
//!     cargo test --release --test probe_class_data -- --nocapture
//! ```

use droidsaw_dex::decode::parse_class_data;

#[test]
fn probe_class_data() {
    let Ok(path) = std::env::var("DROIDSAW_PROBE_CD_INPUT") else {
        eprintln!("DROIDSAW_PROBE_CD_INPUT unset; skipping");
        return;
    };
    let Ok(offsets_csv) = std::env::var("DROIDSAW_PROBE_CD_OFFSETS") else {
        eprintln!("DROIDSAW_PROBE_CD_OFFSETS unset; skipping");
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    eprintln!("\n=== probe_class_data on {path} ===");
    for off_s in offsets_csv.split(',') {
        let Ok(off) = off_s.trim().parse::<u32>() else { continue };
        eprintln!("\n--- offset {off} ({off:#x}) ---");
        // Show ±32 bytes of context.
        let start = (off as usize).saturating_sub(8);
        let end = ((off as usize) + 64).min(bytes.len());
        eprintln!(
            "  ctx [{start}..{end}]: {}",
            bytes[start..end].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
        );
        match parse_class_data(&bytes, off) {
            Ok(cd) => {
                eprintln!("  PARSED OK: static_fields={} instance_fields={} direct_methods={} virtual_methods={}",
                    cd.static_fields.len(), cd.instance_fields.len(),
                    cd.direct_methods.len(), cd.virtual_methods.len());
            }
            Err(e) => {
                eprintln!("  PARSE ERROR: {e:?}");
            }
        }
    }
}
