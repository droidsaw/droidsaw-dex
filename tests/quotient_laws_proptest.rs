//! Quotient-newtype equivalence laws for `ContentEquiv<DexFile>`.
//!
//! `ContentEquiv` (parser.rs) is the round-trip equivalence
//! specification for `emit_dex`. Its `PartialEq` impl IS the spec for
//! "what counts as round-trip equivalent." `parse_emit_parse_structural_
//! equivalence` (in `roundtrip_proptest.rs`) checks PRESERVATION of the
//! class by parse-emit-parse; this file checks that the projection IS
//! a well-formed equivalence relation in the first place.
//!
//! ## Three laws
//!
//! - **Reflexivity** — `x ~ x` for every parse-success `x`.
//! - **Symmetry** — `(a ~ b) == (b ~ a)`. Required by `PartialEq`.
//! - **Transitivity** — `(a ~ b) ∧ (b ~ c) ⇒ a ~ c`. Required by
//!   `PartialEq`. Empty-on-coverage is a real risk on random byte
//!   triples; the transitivity test uses a manual `TestRunner` so the
//!   precondition fire-rate is captured deterministically.
//!
//! ## What this does NOT cover
//!
//! - `Eq` axioms — `ContentEquiv` only impls `PartialEq`.
//! - `DeepContentEquiv` per-BTreeMap-value comparator (out of scope).
//! - Structural `arb_dex_file()` — out of scope.

use droidsaw_dex::parser::{ContentEquiv, DexFile};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::cell::Cell;

mod common;
use common::dex_bytes_strategy;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 1024,
        ..ProptestConfig::default()
    })]

    /// `ContentEquiv(&dex) == ContentEquiv(&dex)` for every parse-
    /// success `dex`. Catches a hypothetical impl that reads from a
    /// non-deterministic source.
    #[test]
    fn content_equiv_reflexive(bytes in dex_bytes_strategy()) {
        let Ok(dex) = DexFile::parse(&bytes, None) else { return Ok(()); };
        prop_assert_eq!(
            ContentEquiv(&dex),
            ContentEquiv(&dex),
            "reflexivity violated: x !~ x"
        );
    }

    /// `(a ~ b) iff (b ~ a)`. Catches asymmetric impls (e.g. a missing
    /// mirror clause in a chained boolean).
    #[test]
    fn content_equiv_symmetric(
        a in dex_bytes_strategy(),
        b in dex_bytes_strategy(),
    ) {
        let Ok(da) = DexFile::parse(&a, None) else { return Ok(()); };
        let Ok(db) = DexFile::parse(&b, None) else { return Ok(()); };
        let ea = ContentEquiv(&da);
        let eb = ContentEquiv(&db);
        let ab = ea == eb;
        let ba = eb == ea;
        prop_assert_eq!(ab, ba, "symmetry violated: ab={}, ba={}", ab, ba);
    }
}

/// Transitivity with deterministic coverage report. Manual
/// `TestRunner` so we own the loop counters; `proptest!` macro hides
/// per-case state behind a closure invocation count we can't read.
#[test]
fn content_equiv_transitive() {
    let config = Config {
        cases: 256,
        max_shrink_iters: 1024,
        ..Config::default()
    };
    let mut runner = TestRunner::new(config);
    let total: Cell<u64> = Cell::new(0);
    let fired: Cell<u64> = Cell::new(0);
    let result = runner.run(
        &(dex_bytes_strategy(), dex_bytes_strategy(), dex_bytes_strategy()),
        |(a, b, c)| {
            total.set(total.get() + 1);
            let Ok(da) = DexFile::parse(&a, None) else { return Ok(()); };
            let Ok(db) = DexFile::parse(&b, None) else { return Ok(()); };
            let Ok(dc) = DexFile::parse(&c, None) else { return Ok(()); };
            let ea = ContentEquiv(&da);
            let eb = ContentEquiv(&db);
            let ec = ContentEquiv(&dc);
            if !(ea == eb && eb == ec) {
                return Ok(());
            }
            fired.set(fired.get() + 1);
            prop_assert!(
                ea == ec,
                "transitivity violated: (a ~ b) ∧ (b ~ c) ∧ ¬(a ~ c)"
            );
            Ok(())
        },
    );
    let total = total.get();
    let fired = fired.get();
    let pct = if total == 0 { 0.0 } else { (fired as f64 / total as f64) * 100.0 };
    eprintln!(
        "[quotient-laws/dex] transitivity precondition fired {fired}/{total} cases ({pct:.2}%)"
    );
    if let Err(e) = result {
        panic!("transitivity proptest failed: {e}");
    }
}
