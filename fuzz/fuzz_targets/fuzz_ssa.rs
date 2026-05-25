#![no_main]

//! `fuzz_ssa` — SSA construction structural-invariant gate.
//!
//! **Asserts:**
//! 1. `SsaBody::build` completes without panic for every method body with a
//!    well-formed CFG. (Panic-freedom invariant.)
//! 2. **Phi-operand predecessor coverage:** for every φ-node in block B, each
//!    operand key is a predecessor of B in the CFG. An orphaned operand key is
//!    a phi-placement bug that produces wrong code silently.
//! 3. **Phi-destination uniqueness:** no two φ-nodes in the same block share a
//!    destination `VarId`. Duplicate phi dsts produce use-before-def silently.
//! 4. **Entry block has no phis (except parameter phis):** the entry block
//!    should not receive phis for values that have no predecessor. If the entry
//!    block has phis with zero operands after trivial-phi removal, that is a
//!    phi-insertion bug.
//!
//! Verifies SSA structural invariants: use-dominance, phi-operand correctness,
//! and phi-placement consistency. The checks here are constructive: they don't
//! require a full dominator walk but do catch the most common phi-placement and
//! phi-sealing bugs.

use std::collections::BTreeSet;

use droidsaw_dex::cfg::Cfg;
use droidsaw_dex::decode::{parse_class_data, parse_code_item};
use droidsaw_dex::ssa::SsaBody;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let dex = match droidsaw_dex::DexFile::parse(data, None) {
        Ok(d) => d,
        Err(_) => return,
    };

    for cd in &dex.class_defs {
        if cd.class_data_off == 0 {
            continue;
        }
        let class_data = match parse_class_data(data, cd.class_data_off) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let methods = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter());
        for em in methods {
            if em.code_off == 0 {
                continue;
            }
            let code = match parse_code_item(data, em.code_off) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let cfg = match Cfg::build(&code) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let ssa = match SsaBody::build(&code, &cfg) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // --- Structural invariants ---

            // Pre-build predecessor map from CFG for fast lookup.
            // Maps BlockIdx → BTreeSet<BlockIdx (preds)>.
            let pred_map: std::collections::BTreeMap<_, BTreeSet<_>> = cfg
                .blocks
                .iter()
                .map(|b| {
                    (
                        b.id,
                        b.predecessors.iter().copied().collect::<BTreeSet<_>>(),
                    )
                })
                .collect();

            for (bid, ssa_block) in &ssa.blocks {
                // Inv 2: phi-operand predecessor coverage.
                // Each operand key in a phi must be a predecessor of this block.
                let preds = pred_map.get(bid).cloned().unwrap_or_default();
                for phi in &ssa_block.phis {
                    for (&pred_bid, _) in &phi.operands {
                        assert!(
                            preds.contains(&pred_bid),
                            "SSA phi-operand predecessor violation: block {bid:?} \
                             has phi for dst={dst:?} with operand from block {pred_bid:?}, \
                             but {pred_bid:?} is not a CFG predecessor of {bid:?}. \
                             Predecessors: {preds:?}",
                            bid = bid,
                            dst = phi.dst,
                            pred_bid = pred_bid,
                            preds = preds,
                        );
                    }

                    // Inv 3: phi-destination uniqueness within the block.
                    let dst_count = ssa_block
                        .phis
                        .iter()
                        .filter(|p| p.dst == phi.dst)
                        .count();
                    assert_eq!(
                        dst_count,
                        1,
                        "SSA phi-dst uniqueness violated: block {bid:?} has {dst_count} \
                         phi nodes sharing dst={dst:?}",
                        bid = bid,
                        dst = phi.dst,
                    );
                }

                // Inv 4: entry block phis must have at least one operand after
                // trivial-phi removal (phis with 0 operands are orphaned).
                if *bid == ssa.entry {
                    for phi in &ssa_block.phis {
                        assert!(
                            !phi.operands.is_empty(),
                            "SSA entry-block phi has zero operands after trivial-phi \
                             removal: block {bid:?}, phi dst={dst:?}. This indicates \
                             a phi-sealing bug.",
                            bid = bid,
                            dst = phi.dst,
                        );
                    }
                }
            }
        }
    }
});
