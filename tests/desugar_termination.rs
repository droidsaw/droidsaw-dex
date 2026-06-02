//! Regression: `sugar::desugar`'s outer fixpoint loop must terminate.
//!
//! A parse failure touching a class's annotation subtree makes its Kotlin
//! detectors return `Indeterminate`, so the class falls through the Kotlin
//! early-return paths to the standard Java decompile path and into
//! `sugar::desugar`. Before the `MAX_DESUGAR_PASSES` cap, a Kotlin-shaped
//! class reached this way could spin the fixpoint loop forever (100% CPU,
//! decompiler hang on adversarial input). The cap guarantees termination —
//! reaching the assertion below means every class decompiled without hanging.

use droidsaw_dex::classes::decompile_class;
use droidsaw_dex::parser::{DexFile, ParseFailure, ParseFailureKind};

#[test]
fn decompile_terminates_when_indeterminate_routes_class_into_desugar() {
    let data = include_bytes!("fixtures/classes_named.dex");
    let mut dex = DexFile::parse(data, None).expect("parse");
    // Planted annotation-subtree parse failure → every Kotlin rendering
    // detector returns Indeterminate → standard Java path → sugar::desugar.
    dex.parse_errors.push(ParseFailure {
        kind: ParseFailureKind::AnnotationItem,
        offset: 0x1000,
    });
    // Decompiling every class must return (not hang). Reaching the assert is
    // the termination guarantee; the count just confirms the fixture is real.
    let count = dex
        .class_defs
        .iter()
        .map(|cd| decompile_class(&dex, data, cd))
        .count();
    assert!(count > 0, "fixture must contain at least one class");
}
