#![no_main]

// Differential oracle fuzz target for DEX CFG construction.
//
// Property: for every DEX method body that the production parser accepts,
// the naive oracle's CfgShape must equal the production Cfg::to_shape().
//
// Any divergence is a silent-wrong-CFG bug — the same class of bug as
// the `ReversedCfg::successors` silent-empty-fallback bug, which caused
// wrong dominators on real DEX bundle CFGs without panicking.
//
// Invariants asserted:
// 1. naive_cfg(code_item_bytes).leaders == production_cfg.to_shape().leaders
// 2. naive_cfg(code_item_bytes).edges == production_cfg.to_shape().edges
// 3. naive_cfg(code_item_bytes).block_instructions == production_cfg.to_shape().block_instructions
//
// Harness design:
// - Input: raw byte slice (arbitrary DEX bytes).
// - Harness runs production DexFile::parse first; skips if parse fails.
// - For each method in the parsed DEX: run production parse_code_item,
//   then both builders on the same input; assert CfgShape equality.
// - Stateless: no internal mutation across fuzz iterations.

use droidsaw_dex::cfg::Cfg;
use droidsaw_dex::cfg_oracle::{naive_cfg, CfgShape};
use droidsaw_dex::decode::{parse_class_data, parse_code_item};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Step 1: production parse. Skip inputs that fail parse.
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

            // Step 2: production CFG builder.
            let prod_cfg = match Cfg::build(&code) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let prod_shape = prod_cfg.to_shape();

            // Step 3: naive oracle.
            // Pass the raw code_item bytes to the oracle (independent decoder).
            let code_off = em.code_off as usize;
            // code_item_bytes: from code_off to end of data (oracle reads only as far as needed)
            let code_item_bytes = match data.get(code_off..) {
                Some(b) => b,
                None => continue,
            };
            let oracle_shape = match naive_cfg(code_item_bytes) {
                Ok(s) => s,
                Err(_) => continue, // oracle parse error on production-accepted input: skip
            };

            // Step 4: assert isomorphism.
            assert_cfg_shapes_equal(&prod_shape, &oracle_shape, em.code_off);
        }
    }
});

fn assert_cfg_shapes_equal(prod: &CfgShape, oracle: &CfgShape, code_off: u32) {
    assert_eq!(
        prod.leaders,
        oracle.leaders,
        "CFG leaders diverge at code_off=0x{code_off:08x}\nproduction: {:?}\noracle: {:?}",
        prod.leaders,
        oracle.leaders,
    );
    assert_eq!(
        prod.edges,
        oracle.edges,
        "CFG edges diverge at code_off=0x{code_off:08x}\nproduction: {:?}\noracle: {:?}",
        prod.edges,
        oracle.edges,
    );
    assert_eq!(
        prod.block_instructions,
        oracle.block_instructions,
        "CFG block_instructions diverge at code_off=0x{code_off:08x}\nproduction: {:?}\noracle: {:?}",
        prod.block_instructions,
        oracle.block_instructions,
    );
}
