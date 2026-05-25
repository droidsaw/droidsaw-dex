//! `fuzz_enum_cross_class` — adversarial cross-class walker gate for
//! the enum-inline-bodies path.
//!
//! `EnumInlineMap::build` iterates across class_defs and reads
//! subclasses' class_data + code items at build time — breaking the
//! "each class is decompiled independently" invariant that previously
//! bounded adversarial blast radius. This target exercises the
//! cross-class path end-to-end on attacker-supplied DEX bytes.
//!
//! **Asserts:**
//! 1. No panic on any input that parses. (Panic-freedom invariant.)
//! 2. **Count monotonicity:** the number of classes decompiled via
//!    `classes_to_decompile` does not exceed `dex.class_defs.len()`.
//!    A count exceeding `class_defs` would indicate the cross-class
//!    walker synthesized phantom entries.
//! 3. **Output is valid UTF-8 with no NUL bytes:** `decompile_class_ext`
//!    returns a Rust `String` (UTF-8 guaranteed); any `\0` byte in the
//!    output is a string-construction bug.
//!
//! Semantic output correctness (inline-body fidelity) is NOT asserted
//! — a hostile DEX may produce compile_fail output via the `bailed` /
//! `unsafe_to_inline` fallback channels.
//!
//! Seed corpus under `fuzz/corpus/fuzz_enum_cross_class/` covers 11
//! adversarial shapes:
//!   (1) truncated subclass, (2) cyclic super, (3) missing code_item,
//!   (4) duplicate constant backing, (5) unsafe-body ref, (6) oversized
//!   constant count, (7) zero-method subclass, (8) constant-without-
//!   subclass, (9) masquerade (super is non-enum), (10) UAF-adjacent
//!   cross-scan mutation, (11) aggregate-decompile-depth trigger.
//!
//! Upgraded from panic-only to structural-invariant.

#![no_main]

use libfuzzer_sys::fuzz_target;

use droidsaw_dex::classes::{
    classes_to_decompile, decompile_class_ext, EnumInlineMap, TypeToClassDefMap,
};
use droidsaw_dex::parser::DexFile;

fuzz_target!(|data: &[u8]| {
    let Ok(dex) = DexFile::parse(data, None) else {
        return;
    };
    let class_def_count = dex.class_defs.len();
    let ttm = TypeToClassDefMap::build(&dex);
    let inlines = EnumInlineMap::build(&dex, data, &ttm);
    let mut decompiled_count: usize = 0;
    for (_, class_def) in classes_to_decompile(&dex) {
        let output = decompile_class_ext(&dex, data, class_def, Some(&inlines), Some(&ttm));

        // Inv 3: output string must not contain NUL bytes.
        assert!(
            !output.contains('\0'),
            "decompile_class_ext returned a string containing NUL bytes — \
             string-construction bug in decompiler output",
        );

        decompiled_count += 1;
    }

    // Inv 2: classes decompiled must not exceed class_defs count.
    assert!(
        decompiled_count <= class_def_count,
        "decompiled {} classes but class_defs.len() == {} — \
         cross-class walker produced phantom entries",
        decompiled_count,
        class_def_count,
    );
});
