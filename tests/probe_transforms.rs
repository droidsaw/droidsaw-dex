//! Print `EmitOutput.applied_transformations` for a single DEX under
//! default emit (`all3` preserve flags, no `preserve_input_checksums`).
//! Used to gauge the typed-transform attribution discipline:
//!   - byte_identical ⇒ applied_transformations.is_empty()
//!   - input_checksums_canonical == false ⇒ InputChecksumNormalized
//!     in applied_transformations
//!
//! ```bash
//! DROIDSAW_PROBE_TX_INPUT=/tmp/foo.dex \
//!     cargo test --release --test probe_transforms -- --nocapture
//! ```

use droidsaw_dex::emit_dex::{emit_dex_collect, EmitConfig};
use droidsaw_dex::parser::DexFile;

#[test]
fn probe_transforms() {
    let Ok(path) = std::env::var("DROIDSAW_PROBE_TX_INPUT") else {
        eprintln!("DROIDSAW_PROBE_TX_INPUT unset; skipping");
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let dex = DexFile::parse(&bytes, None).unwrap();
    let cfg = EmitConfig {
        preserve_data_section_layout: true,
        preserve_map_list_order: true,
        preserve_encoded_value_width: true,
        ..Default::default()
    };
    let out = emit_dex_collect(&dex, &cfg).unwrap();
    let byte_id = out.bytes == bytes;
    eprintln!(
        "{} | input_checksums_canonical={} | byte_id_default={} | transforms={:?}",
        path,
        dex.input_checksums_canonical,
        byte_id,
        out.applied_transformations,
    );
}
