//! Main-package isolation — anti-vacuity against real R8 bytes.
//!
//! `tests/fixtures/r8/block_outlining/artifacts/classes.dex` is a real
//! R8 9.0 release build of package `fox.droidsaw.r8fixture`. R8 kept the
//! `-keep`'d entry point `Lfox/droidsaw/r8fixture/Main;` under the app
//! prefix and flattened the synthetic helper to the root package as
//! `La/a;`. So:
//!   - the all-packages obfuscation stratum counts the flattened `La/a;`,
//!   - the main-package stratum counts only the retained `Main` (== 1),
//!
//! which demonstrates (1) the two strata diverge on real obfuscation and
//! (2) the obfuscated class has structurally left the app namespace —
//! exactly why per-app obfuscation is *not* measured by prefix isolation
//! (per-app obfuscation is therefore a deferred signal).

use droidsaw_dex::obfuscation_features::extract_with_package;
use droidsaw_dex::parser::DexFile;

const BLOCK_OUTLINING: &[u8] =
    include_bytes!("fixtures/r8/block_outlining/artifacts/classes.dex");

#[test]
fn main_package_isolation_is_non_vacuous_on_real_r8_output() {
    let dex = DexFile::parse(BLOCK_OUTLINING, None).expect("block_outlining fixture parses");
    let feats = extract_with_package(&dex, Some("fox.droidsaw.r8fixture"));
    let retained = feats.main_package_class_count.unwrap_or(0);

    // Non-vacuity + Option contract: a non-empty package ⇒ Some(n), and
    // the structural retention count fires on real bytes (the `-keep`'d
    // classes that stayed under the app prefix).
    assert!(
        matches!(feats.main_package_class_count, Some(n) if n >= 1),
        "expected Some(>=1) retained app-prefixed class, got {:?}",
        feats.main_package_class_count
    );

    // Real obfuscation IS present: the synthetic helper R8 flattened to
    // `La/a;` is counted by the all-packages obfuscation stratum.
    assert!(
        feats.obfuscated_class_count >= 1,
        "expected the flattened La/a; in the all-packages obfuscated count, got {}",
        feats.obfuscated_class_count
    );

    // The two strata DIVERGE: at least one class lives outside the app
    // prefix — that is exactly where R8 flattened the obfuscated helper,
    // demonstrating per-app obfuscation is not measurable by prefix
    // isolation (the evidence has left the namespace). This is the
    // deferred-signal rationale, proven on real bytes.
    assert!(
        feats.class_count > retained,
        "a flattened obfuscated class must sit outside the app prefix \
         (class_count {} should exceed retained {})",
        feats.class_count,
        retained
    );
}
