//! Compute Adler-32(input[12..]) + SHA-1(input[32..]) and compare to
//! the values stored in the input header. If they don't match, the
//! input has non-canonical checksums (and our round-trip can't match
//! without copying input checksums verbatim).
//!
//! ```bash
//! DROIDSAW_CHECKSUM_AUDIT_INPUT=/tmp/foo.dex \
//!     cargo test --release --test checksum_audit -- --nocapture
//! ```

use sha1::{Digest, Sha1};

#[test]
fn checksum_audit() {
    let Ok(path) = std::env::var("DROIDSAW_CHECKSUM_AUDIT_INPUT") else {
        eprintln!("DROIDSAW_CHECKSUM_AUDIT_INPUT unset; skipping");
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let stored_adler = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let stored_sha: [u8; 20] = bytes[12..32].try_into().unwrap();
    let computed_adler = adler2::adler32_slice(&bytes[12..]);
    let mut sha = Sha1::new();
    sha.update(&bytes[32..]);
    let computed_sha = sha.finalize();
    eprintln!("\n=== checksum audit: {path} ===");
    eprintln!("file_size = {}", bytes.len());
    eprintln!("stored Adler-32   = {stored_adler:#010x}");
    eprintln!("computed Adler-32 = {computed_adler:#010x}");
    eprintln!("Adler-32 match: {}", stored_adler == computed_adler);
    eprintln!(
        "stored SHA-1   = {}",
        stored_sha.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    eprintln!(
        "computed SHA-1 = {}",
        computed_sha.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    eprintln!("SHA-1 match: {}", stored_sha == computed_sha.as_slice());
}
