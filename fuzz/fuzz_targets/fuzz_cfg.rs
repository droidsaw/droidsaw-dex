#![no_main]

//! `fuzz_cfg` — CFG construction structural-invariant gate.
//!
//! **Asserts:**
//! 1. `Cfg::build` completes without panic for every method body the DEX
//!    parser accepts. (Panic-freedom invariant.)
//! 2. **CFG pred/succ symmetry:** for every edge A → B in the resulting
//!    graph, B appears in A's successors **and** A appears in B's
//!    predecessors. A one-way edge is a CFG-builder bug that downstream
//!    SSA and structuring phases silently miscompile.
//!
//! Verifies CFG structural invariants: panic-freedom and predecessor/successor
//! edge symmetry.

use droidsaw_dex::cfg::Cfg;
use droidsaw_dex::decode::{parse_class_data, parse_code_item};
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

            // Structural invariant: pred/succ symmetry.
            // For every edge A → B: B ∈ A.successors AND A ∈ B.predecessors.
            for block_a in &cfg.blocks {
                for edge in &block_a.successors {
                    let bid_b = edge.target;
                    // Successor target must be a valid block index.
                    let block_b = cfg.blocks.get(bid_b.0 as usize).expect(
                        "successor BlockIdx out of range — CFG-builder bug: \
                         minted a BlockIdx that exceeds blocks.len()",
                    );
                    assert!(
                        block_b.predecessors.contains(&block_a.id),
                        "CFG pred/succ symmetry violated: block {} has successor {} \
                         but {} does not list {} as a predecessor",
                        block_a.id.0,
                        bid_b.0,
                        bid_b.0,
                        block_a.id.0,
                    );
                }
            }
        }
    }
});
